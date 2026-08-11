#include "fission_skia.h"
#include "fission_skia_internal.h"

#include "include/core/SkColorSpace.h"
#include "include/core/SkData.h"
#include "include/core/SkGraphics.h"
#include "include/core/SkImageInfo.h"
#include "include/core/SkPictureRecorder.h"
#include "include/core/SkStream.h"
#include "include/codec/SkCodec.h"
#include "include/codec/SkEncodedOrigin.h"

#include <algorithm>
#include <limits>
#include <memory>
#include <mutex>
#include <new>
#include <thread>
#include <tuple>
#include <utility>

#ifndef FISSION_SKIA_REVISION
#define FISSION_SKIA_REVISION "unknown"
#endif

#ifndef FISSION_SKIA_BUILD_PROFILE
#define FISSION_SKIA_BUILD_PROFILE "native-raster"
#endif

namespace {

constexpr uint64_t kBaseFeatureBits =
    FISSION_SKIA_FEATURE_RASTER_SURFACE |
    FISSION_SKIA_FEATURE_BASIC_FRAME |
    FISSION_SKIA_FEATURE_RGBA_READBACK |
    FISSION_SKIA_FEATURE_STRUCTURED_ERRORS |
    FISSION_SKIA_FEATURE_THREAD_AFFINITY |
    FISSION_SKIA_FEATURE_MEMORY_PRESSURE |
    FISSION_SKIA_FEATURE_PAINT_STATE |
    FISSION_SKIA_FEATURE_PARAGRAPH |
    FISSION_SKIA_FEATURE_OPACITY_LAYER |
    FISSION_SKIA_FEATURE_IMAGE_DECODE |
    FISSION_SKIA_FEATURE_BACKDROP_BLUR |
    FISSION_SKIA_FEATURE_SVG_DOCUMENT |
    FISSION_SKIA_FEATURE_RETAINED_PICTURE;

#if FISSION_SKIA_ENABLE_GANESH_VULKAN
constexpr uint64_t kGaneshFeatureBits =
    FISSION_SKIA_FEATURE_GANESH | FISSION_SKIA_FEATURE_VULKAN |
    FISSION_SKIA_FEATURE_NATIVE_PRESENTATION;
#else
constexpr uint64_t kGaneshFeatureBits = 0;
#endif

constexpr uint64_t kFeatureBits = kBaseFeatureBits | kGaneshFeatureBits;

}  // namespace

using fission::skia::bridge::ContextBackend;
using fission::skia::bridge::ContextState;
using fission::skia::bridge::EngineState;
using fission::skia::bridge::ImageState;
using fission::skia::bridge::PictureState;
using fission::skia::bridge::SurfaceBackend;
using fission::skia::bridge::SurfaceState;
using fission::skia::bridge::SvgDocumentState;
using fission::skia::bridge::check_owner;
using fission::skia::bridge::clear_error;
using fission::skia::bridge::copy_text;
using fission::skia::bridge::fail;
using fission::skia::bridge::finite;
using fission::skia::bridge::next_handle;
using fission::skia::bridge::play_frame;
using fission::skia::bridge::registry;
using fission::skia::bridge::sk_rect;
using fission::skia::bridge::valid_native_window;
using fission::skia::bridge::valid_non_empty_rect;
using fission::skia::bridge::valid_svg_source;
using fission::skia::bridge::validate_frame;
using fission::skia::bridge::write_image_info;
#if FISSION_SKIA_ENABLE_GANESH_VULKAN
using fission::skia::bridge::cancel_ganesh_frame;
#endif

extern "C" {

fission_skia_status_t fission_skia_get_abi_info(
    fission_skia_abi_info_t* out_info,
    fission_skia_error_t* out_error) {
    if (out_info == nullptr || out_info->struct_size != sizeof(fission_skia_abi_info_t)) {
        return fail(FISSION_SKIA_STATUS_INVALID_ARGUMENT, "get_abi_info",
                    "output is null or has an incompatible layout", out_error);
    }
    out_info->abi_version = FISSION_SKIA_ABI_VERSION;
    out_info->feature_bits = kFeatureBits;
    copy_text(out_info->skia_revision, sizeof(out_info->skia_revision), FISSION_SKIA_REVISION);
    copy_text(out_info->build_profile, sizeof(out_info->build_profile), FISSION_SKIA_BUILD_PROFILE);
    clear_error(out_error);
    return FISSION_SKIA_STATUS_OK;
}

fission_skia_status_t fission_skia_engine_create(
    const fission_skia_engine_config_t* config,
    fission_skia_engine_handle_t* out_engine,
    fission_skia_error_t* out_error) {
    if (config == nullptr || config->struct_size != sizeof(fission_skia_engine_config_t) ||
        out_engine == nullptr) {
        return fail(FISSION_SKIA_STATUS_INVALID_ARGUMENT, "engine_create",
                    "configuration or output is invalid", out_error);
    }
    *out_engine = 0;
    if (config->expected_abi_version != FISSION_SKIA_ABI_VERSION) {
        return fail(FISSION_SKIA_STATUS_ABI_MISMATCH, "engine_create",
                    "requested bridge ABI does not match this library", out_error);
    }
    if ((config->required_feature_bits & ~kFeatureBits) != 0) {
        return fail(FISSION_SKIA_STATUS_UNSUPPORTED, "engine_create",
                    "requested bridge features are not available", out_error);
    }
    auto state = std::unique_ptr<EngineState>(new (std::nothrow) EngineState{});
    if (!state) {
        return fail(FISSION_SKIA_STATUS_OUT_OF_MEMORY, "engine_create",
                    "could not allocate engine state", out_error);
    }
    state->owner = std::this_thread::get_id();
    const auto handle = next_handle();
    {
        std::lock_guard<std::mutex> lock(registry().mutex);
        registry().engines.emplace(handle, std::move(state));
    }
    *out_engine = handle;
    clear_error(out_error);
    return FISSION_SKIA_STATUS_OK;
}

fission_skia_status_t fission_skia_engine_destroy(
    fission_skia_engine_handle_t engine,
    fission_skia_error_t* out_error) {
    std::lock_guard<std::mutex> lock(registry().mutex);
    const auto found = registry().engines.find(engine);
    if (found == registry().engines.end()) {
        return fail(FISSION_SKIA_STATUS_INVALID_HANDLE, "engine_destroy",
                    "engine handle is not live", out_error);
    }
    auto status = check_owner(*found->second, "engine_destroy", out_error);
    if (status != FISSION_SKIA_STATUS_OK) return status;
    if (found->second->live_contexts != 0) {
        return fail(FISSION_SKIA_STATUS_INVALID_STATE, "engine_destroy",
                    "engine still owns live contexts", out_error);
    }
    registry().engines.erase(found);
    clear_error(out_error);
    return FISSION_SKIA_STATUS_OK;
}

fission_skia_status_t fission_skia_context_create_raster(
    fission_skia_engine_handle_t engine,
    fission_skia_context_handle_t* out_context,
    fission_skia_error_t* out_error) {
    if (out_context == nullptr) {
        return fail(FISSION_SKIA_STATUS_INVALID_ARGUMENT, "context_create_raster",
                    "context output is null", out_error);
    }
    *out_context = 0;
    std::lock_guard<std::mutex> lock(registry().mutex);
    const auto parent = registry().engines.find(engine);
    if (parent == registry().engines.end()) {
        return fail(FISSION_SKIA_STATUS_INVALID_HANDLE, "context_create_raster",
                    "engine handle is not live", out_error);
    }
    auto status = check_owner(*parent->second, "context_create_raster", out_error);
    if (status != FISSION_SKIA_STATUS_OK) return status;
    auto state = std::unique_ptr<ContextState>(new (std::nothrow) ContextState{});
    if (!state) {
        return fail(FISSION_SKIA_STATUS_OUT_OF_MEMORY, "context_create_raster",
                    "could not allocate context state", out_error);
    }
    state->owner = std::this_thread::get_id();
    state->engine = engine;
    state->backend = ContextBackend::kRaster;
    const auto handle = next_handle();
    registry().contexts.emplace(handle, std::move(state));
    parent->second->live_contexts += 1;
    *out_context = handle;
    clear_error(out_error);
    return FISSION_SKIA_STATUS_OK;
}

fission_skia_status_t fission_skia_context_create_ganesh_vulkan(
    fission_skia_engine_handle_t engine,
    const fission_skia_native_window_t* compatible_window,
    fission_skia_context_handle_t* out_context,
    fission_skia_error_t* out_error) {
    if (out_context == nullptr || !valid_native_window(compatible_window)) {
        return fail(FISSION_SKIA_STATUS_INVALID_ARGUMENT,
                    "context_create_ganesh_vulkan",
                    "context output or native window descriptor is invalid", out_error);
    }
    *out_context = 0;
    std::lock_guard<std::mutex> lock(registry().mutex);
    const auto parent = registry().engines.find(engine);
    if (parent == registry().engines.end()) {
        return fail(FISSION_SKIA_STATUS_INVALID_HANDLE,
                    "context_create_ganesh_vulkan",
                    "engine handle is not live", out_error);
    }
    auto status = check_owner(*parent->second, "context_create_ganesh_vulkan", out_error);
    if (status != FISSION_SKIA_STATUS_OK) return status;
#if FISSION_SKIA_ENABLE_GANESH_VULKAN
    std::unique_ptr<fission::skia::ganesh::VulkanContext> ganesh;
    const auto result = fission::skia::ganesh::VulkanContext::create(
        *compatible_window, &ganesh);
    if (!result.ok()) {
        return fail(result.status, "context_create_ganesh_vulkan", result.message,
                    out_error);
    }
    if (!ganesh) {
        return fail(FISSION_SKIA_STATUS_INTERNAL, "context_create_ganesh_vulkan",
                    "Ganesh Vulkan context creation returned no context", out_error);
    }
    auto state = std::unique_ptr<ContextState>(new (std::nothrow) ContextState{});
    if (!state) {
        return fail(FISSION_SKIA_STATUS_OUT_OF_MEMORY, "context_create_ganesh_vulkan",
                    "could not allocate context state", out_error);
    }
    state->owner = std::this_thread::get_id();
    state->engine = engine;
    state->backend = ContextBackend::kGaneshVulkan;
    state->ganesh = std::move(ganesh);
    const auto handle = next_handle();
    registry().contexts.emplace(handle, std::move(state));
    parent->second->live_contexts += 1;
    *out_context = handle;
    clear_error(out_error);
    return FISSION_SKIA_STATUS_OK;
#else
    return fail(FISSION_SKIA_STATUS_UNSUPPORTED, "context_create_ganesh_vulkan",
                "this bridge profile does not implement Ganesh Vulkan resources",
                out_error);
#endif
}

fission_skia_status_t fission_skia_context_trim_memory(
    fission_skia_context_handle_t context,
    uint32_t pressure,
    fission_skia_error_t* out_error) {
    std::lock_guard<std::mutex> lock(registry().mutex);
    const auto found = registry().contexts.find(context);
    if (found == registry().contexts.end()) {
        return fail(FISSION_SKIA_STATUS_INVALID_HANDLE, "context_trim_memory",
                    "context handle is not live", out_error);
    }
    auto status = check_owner(*found->second, "context_trim_memory", out_error);
    if (status != FISSION_SKIA_STATUS_OK) return status;
    if (pressure != FISSION_SKIA_MEMORY_PRESSURE_MODERATE &&
        pressure != FISSION_SKIA_MEMORY_PRESSURE_CRITICAL) {
        return fail(FISSION_SKIA_STATUS_INVALID_ARGUMENT, "context_trim_memory",
                    "memory pressure value is unknown", out_error);
    }
#if FISSION_SKIA_ENABLE_GANESH_VULKAN
    if (found->second->backend == ContextBackend::kGaneshVulkan) {
        if (!found->second->ganesh) {
            return fail(FISSION_SKIA_STATUS_INTERNAL, "context_trim_memory",
                        "Ganesh context state has no Vulkan context", out_error);
        }
        const auto result = found->second->ganesh->trim_memory(pressure);
        if (!result.ok()) {
            return fail(result.status, "context_trim_memory", result.message, out_error);
        }
    }
#endif
    SkGraphics::PurgeAllCaches();
    clear_error(out_error);
    return FISSION_SKIA_STATUS_OK;
}

fission_skia_status_t fission_skia_context_destroy(
    fission_skia_context_handle_t context,
    fission_skia_error_t* out_error) {
    std::lock_guard<std::mutex> lock(registry().mutex);
    const auto found = registry().contexts.find(context);
    if (found == registry().contexts.end()) {
        return fail(FISSION_SKIA_STATUS_INVALID_HANDLE, "context_destroy",
                    "context handle is not live", out_error);
    }
    auto status = check_owner(*found->second, "context_destroy", out_error);
    if (status != FISSION_SKIA_STATUS_OK) return status;
    if (found->second->live_surfaces != 0) {
        return fail(FISSION_SKIA_STATUS_INVALID_STATE, "context_destroy",
                    "context still owns live surfaces", out_error);
    }
    const auto engine = found->second->engine;
    registry().contexts.erase(found);
    const auto parent = registry().engines.find(engine);
    if (parent != registry().engines.end()) parent->second->live_contexts -= 1;
    clear_error(out_error);
    return FISSION_SKIA_STATUS_OK;
}

fission_skia_status_t fission_skia_surface_create_raster(
    fission_skia_context_handle_t context,
    uint32_t width,
    uint32_t height,
    fission_skia_surface_handle_t* out_surface,
    fission_skia_error_t* out_error) {
    if (out_surface == nullptr || width == 0 || height == 0 ||
        width > static_cast<uint32_t>(std::numeric_limits<int>::max()) ||
        height > static_cast<uint32_t>(std::numeric_limits<int>::max())) {
        return fail(FISSION_SKIA_STATUS_INVALID_ARGUMENT, "surface_create_raster",
                    "surface output or dimensions are invalid", out_error);
    }
    *out_surface = 0;
    std::lock_guard<std::mutex> lock(registry().mutex);
    const auto parent = registry().contexts.find(context);
    if (parent == registry().contexts.end()) {
        return fail(FISSION_SKIA_STATUS_INVALID_HANDLE, "surface_create_raster",
                    "context handle is not live", out_error);
    }
    auto status = check_owner(*parent->second, "surface_create_raster", out_error);
    if (status != FISSION_SKIA_STATUS_OK) return status;
    if (parent->second->backend != ContextBackend::kRaster) {
        return fail(FISSION_SKIA_STATUS_INVALID_STATE, "surface_create_raster",
                    "context is not a raster context", out_error);
    }
    auto color_space = SkColorSpace::MakeSRGB();
    auto surface = SkSurfaces::Raster(
        SkImageInfo::MakeN32Premul(static_cast<int>(width), static_cast<int>(height),
                                  color_space));
    if (!surface) {
        return fail(FISSION_SKIA_STATUS_OUT_OF_MEMORY, "surface_create_raster",
                    "Skia could not allocate the raster surface", out_error);
    }
    auto state = std::unique_ptr<SurfaceState>(new (std::nothrow) SurfaceState{});
    if (!state) {
        return fail(FISSION_SKIA_STATUS_OUT_OF_MEMORY, "surface_create_raster",
                    "could not allocate surface state", out_error);
    }
    state->owner = std::this_thread::get_id();
    state->context = context;
    state->width = width;
    state->height = height;
    state->backend = SurfaceBackend::kRaster;
    state->surface = std::move(surface);
    const auto handle = next_handle();
    registry().surfaces.emplace(handle, std::move(state));
    parent->second->live_surfaces += 1;
    *out_surface = handle;
    clear_error(out_error);
    return FISSION_SKIA_STATUS_OK;
}

fission_skia_status_t fission_skia_surface_create_ganesh(
    fission_skia_context_handle_t context,
    const fission_skia_native_window_t* window,
    uint32_t width,
    uint32_t height,
    fission_skia_surface_handle_t* out_surface,
    fission_skia_error_t* out_error) {
    if (out_surface == nullptr || !valid_native_window(window) ||
        width > static_cast<uint32_t>(std::numeric_limits<int>::max()) ||
        height > static_cast<uint32_t>(std::numeric_limits<int>::max())) {
        return fail(FISSION_SKIA_STATUS_INVALID_ARGUMENT, "surface_create_ganesh",
                    "surface output, native window, or dimensions are invalid", out_error);
    }
    *out_surface = 0;
    std::lock_guard<std::mutex> lock(registry().mutex);
    const auto parent = registry().contexts.find(context);
    if (parent == registry().contexts.end()) {
        return fail(FISSION_SKIA_STATUS_INVALID_HANDLE, "surface_create_ganesh",
                    "context handle is not live", out_error);
    }
    auto status = check_owner(*parent->second, "surface_create_ganesh", out_error);
    if (status != FISSION_SKIA_STATUS_OK) return status;
#if FISSION_SKIA_ENABLE_GANESH_VULKAN
    if (parent->second->backend != ContextBackend::kGaneshVulkan ||
        !parent->second->ganesh) {
        return fail(FISSION_SKIA_STATUS_INVALID_STATE, "surface_create_ganesh",
                    "context is not a live Ganesh Vulkan context", out_error);
    }
    std::unique_ptr<fission::skia::ganesh::VulkanSurface> ganesh;
    const auto result = fission::skia::ganesh::VulkanSurface::create(
        *parent->second->ganesh, *window, width, height, &ganesh);
    if (!result.ok()) {
        return fail(result.status, "surface_create_ganesh", result.message, out_error);
    }
    if (!ganesh) {
        return fail(FISSION_SKIA_STATUS_INTERNAL, "surface_create_ganesh",
                    "Ganesh surface creation returned no surface", out_error);
    }
    auto state = std::unique_ptr<SurfaceState>(new (std::nothrow) SurfaceState{});
    if (!state) {
        return fail(FISSION_SKIA_STATUS_OUT_OF_MEMORY, "surface_create_ganesh",
                    "could not allocate surface state", out_error);
    }
    state->owner = std::this_thread::get_id();
    state->context = context;
    state->width = width;
    state->height = height;
    state->backend = SurfaceBackend::kGaneshVulkan;
    state->ganesh = std::move(ganesh);
    const auto handle = next_handle();
    registry().surfaces.emplace(handle, std::move(state));
    parent->second->live_surfaces += 1;
    *out_surface = handle;
    clear_error(out_error);
    return FISSION_SKIA_STATUS_OK;
#else
    return fail(FISSION_SKIA_STATUS_UNSUPPORTED, "surface_create_ganesh",
                "this bridge profile does not implement Ganesh native surfaces",
                out_error);
#endif
}

fission_skia_status_t fission_skia_surface_resize_ganesh(
    fission_skia_surface_handle_t surface,
    const fission_skia_native_window_t* window,
    uint32_t width,
    uint32_t height,
    fission_skia_error_t* out_error) {
    if (!valid_native_window(window) ||
        width > static_cast<uint32_t>(std::numeric_limits<int>::max()) ||
        height > static_cast<uint32_t>(std::numeric_limits<int>::max())) {
        return fail(FISSION_SKIA_STATUS_INVALID_ARGUMENT, "surface_resize_ganesh",
                    "native window or dimensions are invalid", out_error);
    }
    std::lock_guard<std::mutex> lock(registry().mutex);
    const auto found = registry().surfaces.find(surface);
    if (found == registry().surfaces.end()) {
        return fail(FISSION_SKIA_STATUS_INVALID_HANDLE, "surface_resize_ganesh",
                    "surface handle is not live", out_error);
    }
    auto status = check_owner(*found->second, "surface_resize_ganesh", out_error);
    if (status != FISSION_SKIA_STATUS_OK) return status;
#if FISSION_SKIA_ENABLE_GANESH_VULKAN
    if (found->second->backend != SurfaceBackend::kGaneshVulkan ||
        !found->second->ganesh) {
        return fail(FISSION_SKIA_STATUS_INVALID_STATE, "surface_resize_ganesh",
                    "surface is not a Ganesh native surface", out_error);
    }
    const auto result = found->second->ganesh->resize(*window, width, height);
    if (!result.ok()) {
        return fail(result.status, "surface_resize_ganesh", result.message, out_error);
    }
    found->second->width = width;
    found->second->height = height;
    clear_error(out_error);
    return FISSION_SKIA_STATUS_OK;
#else
    return fail(FISSION_SKIA_STATUS_INVALID_STATE, "surface_resize_ganesh",
                "surface is not a Ganesh native surface", out_error);
#endif
}

fission_skia_status_t fission_skia_surface_execute_frame(
    fission_skia_surface_handle_t surface,
    const fission_skia_frame_t* frame,
    fission_skia_error_t* out_error) {
    auto status = validate_frame(frame, false, out_error);
    if (status != FISSION_SKIA_STATUS_OK) return status;
    std::lock_guard<std::mutex> lock(registry().mutex);
    const auto found = registry().surfaces.find(surface);
    if (found == registry().surfaces.end()) {
        return fail(FISSION_SKIA_STATUS_INVALID_HANDLE, "execute_frame",
                    "surface handle is not live", out_error);
    }
    status = check_owner(*found->second, "execute_frame", out_error);
    if (status != FISSION_SKIA_STATUS_OK) return status;
#if FISSION_SKIA_ENABLE_GANESH_VULKAN
    if (found->second->backend == SurfaceBackend::kGaneshVulkan) {
        if (!found->second->ganesh) {
            return fail(FISSION_SKIA_STATUS_INTERNAL, "execute_frame",
                        "Ganesh surface state has no Vulkan surface", out_error);
        }
        auto acquired = found->second->ganesh->begin_frame();
        if (!acquired.result.ok()) {
            return fail(acquired.result.status, "execute_frame",
                        acquired.result.message, out_error);
        }
        if (acquired.canvas == nullptr) {
            status = fail(FISSION_SKIA_STATUS_INTERNAL, "execute_frame",
                          "Ganesh began a frame without a Skia canvas", out_error);
            return cancel_ganesh_frame(*found->second->ganesh, status, out_error);
        }
        status = play_frame(
            acquired.canvas, found->second.get(), *frame, "execute_frame", out_error);
        if (status != FISSION_SKIA_STATUS_OK) {
            return cancel_ganesh_frame(*found->second->ganesh, status, out_error);
        }
        const auto finish = found->second->ganesh->finish_frame();
        if (!finish.ok()) {
            status = fail(finish.status, "execute_frame", finish.message, out_error);
            return cancel_ganesh_frame(*found->second->ganesh, status, out_error);
        }
        clear_error(out_error);
        return FISSION_SKIA_STATUS_OK;
    }
#endif
    if (found->second->backend != SurfaceBackend::kRaster || !found->second->surface) {
        return fail(FISSION_SKIA_STATUS_INTERNAL, "execute_frame",
                    "raster surface state has no Skia surface", out_error);
    }
    auto* canvas = found->second->surface->getCanvas();
    if (canvas == nullptr) {
        return fail(FISSION_SKIA_STATUS_SURFACE_LOST, "execute_frame",
                    "raster surface has no canvas", out_error);
    }
    status = play_frame(canvas, found->second.get(), *frame, "execute_frame", out_error);
    if (status != FISSION_SKIA_STATUS_OK) return status;
    clear_error(out_error);
    return FISSION_SKIA_STATUS_OK;
}

fission_skia_status_t fission_skia_surface_read_pixels_rgba8888(
    fission_skia_surface_handle_t surface,
    const fission_skia_pixel_rect_t* source_rect,
    uint8_t* destination,
    size_t destination_length,
    size_t destination_row_bytes,
    size_t* out_required_length,
    fission_skia_error_t* out_error) {
    if (source_rect == nullptr || out_required_length == nullptr ||
        source_rect->width == 0 || source_rect->height == 0 ||
        source_rect->x < 0 || source_rect->y < 0) {
        return fail(FISSION_SKIA_STATUS_INVALID_ARGUMENT, "read_pixels_rgba8888",
                    "readback rectangle or required-length output is invalid", out_error);
    }
    const size_t tight_row = static_cast<size_t>(source_rect->width) * 4;
    if (destination_row_bytes < tight_row ||
        source_rect->height > std::numeric_limits<size_t>::max() / destination_row_bytes) {
        return fail(FISSION_SKIA_STATUS_INVALID_ARGUMENT, "read_pixels_rgba8888",
                    "readback row stride is invalid", out_error);
    }
    const size_t required = destination_row_bytes * source_rect->height;
    *out_required_length = required;
    if (destination == nullptr || destination_length < required) {
        return fail(FISSION_SKIA_STATUS_INVALID_ARGUMENT, "read_pixels_rgba8888",
                    "readback destination is too small", out_error);
    }
    std::lock_guard<std::mutex> lock(registry().mutex);
    const auto found = registry().surfaces.find(surface);
    if (found == registry().surfaces.end()) {
        return fail(FISSION_SKIA_STATUS_INVALID_HANDLE, "read_pixels_rgba8888",
                    "surface handle is not live", out_error);
    }
    auto status = check_owner(*found->second, "read_pixels_rgba8888", out_error);
    if (status != FISSION_SKIA_STATUS_OK) return status;
    const uint64_t right = static_cast<uint64_t>(source_rect->x) + source_rect->width;
    const uint64_t bottom = static_cast<uint64_t>(source_rect->y) + source_rect->height;
    if (right > found->second->width || bottom > found->second->height) {
        return fail(FISSION_SKIA_STATUS_INVALID_ARGUMENT, "read_pixels_rgba8888",
                    "readback rectangle lies outside the surface", out_error);
    }
#if FISSION_SKIA_ENABLE_GANESH_VULKAN
    if (found->second->backend == SurfaceBackend::kGaneshVulkan) {
        if (!found->second->ganesh) {
            return fail(FISSION_SKIA_STATUS_INTERNAL, "read_pixels_rgba8888",
                        "Ganesh surface state has no Vulkan surface", out_error);
        }
        const auto result = found->second->ganesh->read_pixels_rgba8888(
            source_rect->x, source_rect->y, source_rect->width, source_rect->height,
            destination, destination_length, destination_row_bytes);
        if (!result.ok()) {
            return fail(result.status, "read_pixels_rgba8888", result.message, out_error);
        }
        clear_error(out_error);
        return FISSION_SKIA_STATUS_OK;
    }
#endif
    if (found->second->backend != SurfaceBackend::kRaster || !found->second->surface) {
        return fail(FISSION_SKIA_STATUS_INTERNAL, "read_pixels_rgba8888",
                    "raster surface state has no Skia surface", out_error);
    }
    auto srgb = SkColorSpace::MakeSRGB();
    const auto info = SkImageInfo::Make(
        static_cast<int>(source_rect->width), static_cast<int>(source_rect->height),
        kRGBA_8888_SkColorType, kPremul_SkAlphaType, srgb);
    if (!found->second->surface->readPixels(info, destination, destination_row_bytes,
                                            source_rect->x, source_rect->y)) {
        return fail(FISSION_SKIA_STATUS_SURFACE_LOST, "read_pixels_rgba8888",
                    "Skia could not read the requested raster pixels", out_error);
    }
    clear_error(out_error);
    return FISSION_SKIA_STATUS_OK;
}

fission_skia_status_t fission_skia_surface_present(
    fission_skia_surface_handle_t surface,
    fission_skia_error_t* out_error) {
    std::lock_guard<std::mutex> lock(registry().mutex);
    const auto found = registry().surfaces.find(surface);
    if (found == registry().surfaces.end()) {
        return fail(FISSION_SKIA_STATUS_INVALID_HANDLE, "surface_present",
                    "surface handle is not live", out_error);
    }
    auto status = check_owner(*found->second, "surface_present", out_error);
    if (status != FISSION_SKIA_STATUS_OK) return status;
#if FISSION_SKIA_ENABLE_GANESH_VULKAN
    if (found->second->backend == SurfaceBackend::kGaneshVulkan) {
        if (!found->second->ganesh) {
            return fail(FISSION_SKIA_STATUS_INTERNAL, "surface_present",
                        "Ganesh surface state has no Vulkan surface", out_error);
        }
        const auto result = found->second->ganesh->present();
        if (!result.ok()) {
            return fail(result.status, "surface_present", result.message, out_error);
        }
        clear_error(out_error);
        return FISSION_SKIA_STATUS_OK;
    }
#endif
    return fail(FISSION_SKIA_STATUS_INVALID_STATE, "surface_present",
                "surface is not a Ganesh native surface", out_error);
}

fission_skia_status_t fission_skia_surface_destroy(
    fission_skia_surface_handle_t surface,
    fission_skia_error_t* out_error) {
    std::lock_guard<std::mutex> lock(registry().mutex);
    const auto found = registry().surfaces.find(surface);
    if (found == registry().surfaces.end()) {
        return fail(FISSION_SKIA_STATUS_INVALID_HANDLE, "surface_destroy",
                    "surface handle is not live", out_error);
    }
    auto status = check_owner(*found->second, "surface_destroy", out_error);
    if (status != FISSION_SKIA_STATUS_OK) return status;
    const auto context = found->second->context;
    registry().surfaces.erase(found);
    const auto parent = registry().contexts.find(context);
    if (parent != registry().contexts.end()) parent->second->live_surfaces -= 1;
    clear_error(out_error);
    return FISSION_SKIA_STATUS_OK;
}

fission_skia_status_t fission_skia_image_decode_encoded(
    const uint8_t* encoded,
    size_t encoded_length,
    size_t max_decoded_bytes,
    fission_skia_image_handle_t* out_image,
    fission_skia_image_info_t* out_info,
    fission_skia_error_t* out_error) {
    if (encoded == nullptr || encoded_length == 0 || max_decoded_bytes == 0 ||
        out_image == nullptr ||
        out_info == nullptr || out_info->struct_size != sizeof(fission_skia_image_info_t)) {
        return fail(FISSION_SKIA_STATUS_INVALID_ARGUMENT, "image_decode_encoded",
                    "encoded bytes or image outputs are invalid", out_error);
    }
    *out_image = 0;
    out_info->width = 0;
    out_info->height = 0;
    out_info->reserved = 0;
    out_info->approximate_decoded_bytes = 0;

    auto data = SkData::MakeWithoutCopy(encoded, encoded_length);
    if (!data) {
        return fail(FISSION_SKIA_STATUS_OUT_OF_MEMORY, "image_decode_encoded",
                    "Skia could not create the encoded image view", out_error);
    }
    auto codec = SkCodec::MakeFromData(std::move(data));
    if (!codec) {
        return fail(FISSION_SKIA_STATUS_UNSUPPORTED, "image_decode_encoded",
                    "encoded image format is invalid or unavailable in this profile", out_error);
    }
    const SkImageInfo source_info = codec->getInfo();
    int width = source_info.width();
    int height = source_info.height();
    if (SkEncodedOriginSwapsWidthHeight(codec->getOrigin())) {
        std::swap(width, height);
    }
    if (width <= 0 || height <= 0) {
        return fail(FISSION_SKIA_STATUS_INVALID_ARGUMENT, "image_decode_encoded",
                    "encoded image dimensions are invalid", out_error);
    }
    const SkImageInfo decoded_info = SkImageInfo::MakeN32Premul(
        width, height, SkColorSpace::MakeSRGB());
    const size_t approximate_bytes = decoded_info.computeMinByteSize();
    if (SkImageInfo::ByteSizeOverflowed(approximate_bytes) || approximate_bytes == 0) {
        return fail(FISSION_SKIA_STATUS_INVALID_ARGUMENT, "image_decode_encoded",
                    "decoded image byte size overflows this platform", out_error);
    }
    if (approximate_bytes > max_decoded_bytes) {
        return fail(FISSION_SKIA_STATUS_OUT_OF_MEMORY, "image_decode_encoded",
                    "decoded image exceeds the caller byte limit", out_error);
    }
    SkCodec::Options options;
    options.fMaxDecodeMemory = max_decoded_bytes;
    auto [image, result] = codec->getImage(decoded_info, &options);
    if (!image) {
        if (result == SkCodec::kOutOfMemory) {
            return fail(FISSION_SKIA_STATUS_OUT_OF_MEMORY, "image_decode_encoded",
                        "Skia could not allocate decoded image pixels", out_error);
        }
        if (result == SkCodec::kUnimplemented) {
            return fail(FISSION_SKIA_STATUS_UNSUPPORTED, "image_decode_encoded",
                        "the artifact codec cannot decode this image", out_error);
        }
        return fail(FISSION_SKIA_STATUS_INVALID_ARGUMENT, "image_decode_encoded",
                    "the encoded image could not be decoded", out_error);
    }
    if (image->width() != width || image->height() != height ||
        image->imageInfo().computeMinByteSize() != approximate_bytes) {
        return fail(FISSION_SKIA_STATUS_INTERNAL, "image_decode_encoded",
                    "Skia returned image storage that differs from its preflight", out_error);
    }
    auto state = std::unique_ptr<ImageState>(new (std::nothrow) ImageState{});
    if (!state) {
        return fail(FISSION_SKIA_STATUS_OUT_OF_MEMORY, "image_decode_encoded",
                    "could not allocate decoded image state", out_error);
    }
    state->width = static_cast<uint32_t>(width);
    state->height = static_cast<uint32_t>(height);
    state->approximate_decoded_bytes = approximate_bytes;
    state->image = std::move(image);
    const auto image_handle = next_handle();
    {
        std::lock_guard<std::mutex> lock(registry().mutex);
        registry().images.emplace(image_handle, std::move(state));
        write_image_info(*registry().images.at(image_handle), out_info);
    }
    *out_image = image_handle;
    clear_error(out_error);
    return FISSION_SKIA_STATUS_OK;
}

fission_skia_status_t fission_skia_image_get_info(
    fission_skia_image_handle_t image,
    fission_skia_image_info_t* out_info,
    fission_skia_error_t* out_error) {
    if (out_info == nullptr || out_info->struct_size != sizeof(fission_skia_image_info_t)) {
        return fail(FISSION_SKIA_STATUS_INVALID_ARGUMENT, "image_get_info",
                    "image info output is invalid", out_error);
    }
    std::lock_guard<std::mutex> lock(registry().mutex);
    const auto found = registry().images.find(image);
    if (found == registry().images.end()) {
        return fail(FISSION_SKIA_STATUS_INVALID_HANDLE, "image_get_info",
                    "image handle is not live", out_error);
    }
    write_image_info(*found->second, out_info);
    clear_error(out_error);
    return FISSION_SKIA_STATUS_OK;
}

fission_skia_status_t fission_skia_image_destroy(
    fission_skia_image_handle_t image,
    fission_skia_error_t* out_error) {
    std::lock_guard<std::mutex> lock(registry().mutex);
    const auto found = registry().images.find(image);
    if (found == registry().images.end()) {
        return fail(FISSION_SKIA_STATUS_INVALID_HANDLE, "image_destroy",
                    "image handle is not live", out_error);
    }
    registry().images.erase(found);
    clear_error(out_error);
    return FISSION_SKIA_STATUS_OK;
}

fission_skia_status_t fission_skia_svg_document_parse(
    const uint8_t* svg,
    size_t svg_length,
    fission_skia_svg_document_handle_t* out_document,
    fission_skia_error_t* out_error) {
    if (out_document == nullptr || !valid_svg_source(svg, svg_length)) {
        return fail(FISSION_SKIA_STATUS_INVALID_ARGUMENT, "svg_document_parse",
                    "SVG input is empty, oversized, unsafe, or invalid UTF-8", out_error);
    }
    *out_document = 0;

    SkMemoryStream stream(svg, svg_length, false);
    auto document = SkSVGDOM::MakeFromStream(stream);
    if (!document || document->getRoot() == nullptr) {
        return fail(FISSION_SKIA_STATUS_INVALID_ARGUMENT, "svg_document_parse",
                    "input did not parse to an SVG document root", out_error);
    }
    const SkSize intrinsic_size = document->containerSize();
    if (!finite(intrinsic_size.width()) || !finite(intrinsic_size.height()) ||
        intrinsic_size.width() < 0.0f || intrinsic_size.height() < 0.0f) {
        return fail(FISSION_SKIA_STATUS_INVALID_ARGUMENT, "svg_document_parse",
                    "SVG document has an invalid intrinsic size", out_error);
    }

    auto state = std::unique_ptr<SvgDocumentState>(
        new (std::nothrow) SvgDocumentState{});
    if (!state) {
        return fail(FISSION_SKIA_STATUS_OUT_OF_MEMORY, "svg_document_parse",
                    "could not allocate SVG document state", out_error);
    }
    state->intrinsic_size = intrinsic_size;
    state->document = std::move(document);
    const auto handle = next_handle();
    {
        std::lock_guard<std::mutex> lock(registry().mutex);
        registry().svg_documents.emplace(handle, std::move(state));
    }
    *out_document = handle;
    clear_error(out_error);
    return FISSION_SKIA_STATUS_OK;
}

fission_skia_status_t fission_skia_svg_document_destroy(
    fission_skia_svg_document_handle_t document,
    fission_skia_error_t* out_error) {
    std::lock_guard<std::mutex> lock(registry().mutex);
    const auto found = registry().svg_documents.find(document);
    if (found == registry().svg_documents.end()) {
        return fail(FISSION_SKIA_STATUS_INVALID_HANDLE, "svg_document_destroy",
                    "SVG document handle is not live", out_error);
    }
    registry().svg_documents.erase(found);
    clear_error(out_error);
    return FISSION_SKIA_STATUS_OK;
}

fission_skia_status_t fission_skia_picture_record(
    const fission_skia_rect_t* cull_bounds,
    const fission_skia_frame_t* frame,
    fission_skia_picture_handle_t* out_picture,
    fission_skia_error_t* out_error) {
    if (cull_bounds == nullptr || !valid_non_empty_rect(*cull_bounds) ||
        !finite(cull_bounds->x + cull_bounds->width) ||
        !finite(cull_bounds->y + cull_bounds->height) || out_picture == nullptr) {
        return fail(FISSION_SKIA_STATUS_INVALID_ARGUMENT, "picture_record",
                    "picture cull bounds or output are invalid", out_error);
    }
    *out_picture = 0;
    auto status = validate_frame(frame, true, out_error);
    if (status != FISSION_SKIA_STATUS_OK) {
        if (out_error != nullptr &&
            out_error->struct_size == sizeof(fission_skia_error_t)) {
            copy_text(out_error->operation, sizeof(out_error->operation),
                      "picture_record");
        }
        return status;
    }

    std::lock_guard<std::mutex> lock(registry().mutex);
    SkPictureRecorder recorder;
    SkCanvas* canvas = recorder.beginRecording(sk_rect(*cull_bounds));
    if (canvas == nullptr) {
        return fail(FISSION_SKIA_STATUS_OUT_OF_MEMORY, "picture_record",
                    "Skia could not create a picture recording canvas", out_error);
    }
    status = play_frame(canvas, nullptr, *frame, "picture_record", out_error);
    if (status != FISSION_SKIA_STATUS_OK) return status;
    auto picture = recorder.finishRecordingAsPicture();
    if (!picture) {
        return fail(FISSION_SKIA_STATUS_OUT_OF_MEMORY, "picture_record",
                    "Skia could not finish the retained picture", out_error);
    }
    auto state = std::unique_ptr<PictureState>(new (std::nothrow) PictureState{});
    if (!state) {
        return fail(FISSION_SKIA_STATUS_OUT_OF_MEMORY, "picture_record",
                    "could not allocate retained picture state", out_error);
    }
    state->picture = std::move(picture);
    const auto handle = next_handle();
    registry().pictures.emplace(handle, std::move(state));
    *out_picture = handle;
    clear_error(out_error);
    return FISSION_SKIA_STATUS_OK;
}

fission_skia_status_t fission_skia_picture_destroy(
    fission_skia_picture_handle_t picture,
    fission_skia_error_t* out_error) {
    std::lock_guard<std::mutex> lock(registry().mutex);
    const auto found = registry().pictures.find(picture);
    if (found == registry().pictures.end()) {
        return fail(FISSION_SKIA_STATUS_INVALID_HANDLE, "picture_destroy",
                    "picture handle is not live", out_error);
    }
    registry().pictures.erase(found);
    clear_error(out_error);
    return FISSION_SKIA_STATUS_OK;
}

}  // extern "C"
