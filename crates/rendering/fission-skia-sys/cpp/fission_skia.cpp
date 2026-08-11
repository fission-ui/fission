#include "fission_skia.h"

#include "include/core/SkCanvas.h"
#include "include/core/SkColorSpace.h"
#include "include/core/SkGraphics.h"
#include "include/core/SkImageInfo.h"
#include "include/core/SkPaint.h"
#include "include/core/SkPath.h"
#include "include/core/SkSurface.h"

#include <atomic>
#include <cmath>
#include <cstring>
#include <limits>
#include <memory>
#include <mutex>
#include <new>
#include <thread>
#include <unordered_map>

#ifndef FISSION_SKIA_REVISION
#define FISSION_SKIA_REVISION "unknown"
#endif

#ifndef FISSION_SKIA_BUILD_PROFILE
#define FISSION_SKIA_BUILD_PROFILE "native-raster"
#endif

namespace {

constexpr uint64_t kFeatureBits =
    FISSION_SKIA_FEATURE_RASTER_SURFACE |
    FISSION_SKIA_FEATURE_BASIC_FRAME |
    FISSION_SKIA_FEATURE_RGBA_READBACK |
    FISSION_SKIA_FEATURE_STRUCTURED_ERRORS |
    FISSION_SKIA_FEATURE_THREAD_AFFINITY |
    FISSION_SKIA_FEATURE_MEMORY_PRESSURE;

struct EngineState {
    std::thread::id owner;
    uint64_t live_contexts = 0;
};

struct ContextState {
    std::thread::id owner;
    fission_skia_engine_handle_t engine = 0;
    uint64_t live_surfaces = 0;
};

struct SurfaceState {
    std::thread::id owner;
    fission_skia_context_handle_t context = 0;
    uint32_t width = 0;
    uint32_t height = 0;
    sk_sp<SkSurface> surface;
};

struct Registry {
    std::mutex mutex;
    std::unordered_map<uint64_t, std::unique_ptr<EngineState>> engines;
    std::unordered_map<uint64_t, std::unique_ptr<ContextState>> contexts;
    std::unordered_map<uint64_t, std::unique_ptr<SurfaceState>> surfaces;
    std::atomic<uint64_t> next_handle{1};
    std::atomic<uint64_t> next_error{1};
};

Registry& registry() {
    static Registry value;
    return value;
}

uint64_t next_handle() {
    uint64_t handle = registry().next_handle.fetch_add(1, std::memory_order_relaxed);
    return handle == 0
        ? registry().next_handle.fetch_add(1, std::memory_order_relaxed)
        : handle;
}

void copy_text(char* destination, size_t capacity, const char* source) {
    if (capacity == 0) {
        return;
    }
    const size_t length = std::strlen(source);
    const size_t copied = length < capacity - 1 ? length : capacity - 1;
    std::memcpy(destination, source, copied);
    destination[copied] = '\0';
    if (copied + 1 < capacity) {
        std::memset(destination + copied + 1, 0, capacity - copied - 1);
    }
}

void clear_error(fission_skia_error_t* error) {
    if (error == nullptr || error->struct_size != sizeof(fission_skia_error_t)) {
        return;
    }
    error->code = FISSION_SKIA_STATUS_OK;
    error->sequence = 0;
    error->operation[0] = '\0';
    error->message[0] = '\0';
}

fission_skia_status_t fail(
    fission_skia_status_t status,
    const char* operation,
    const char* message,
    fission_skia_error_t* error) {
    if (error != nullptr && error->struct_size == sizeof(fission_skia_error_t)) {
        error->code = status;
        error->sequence = registry().next_error.fetch_add(1, std::memory_order_relaxed);
        copy_text(error->operation, sizeof(error->operation), operation);
        copy_text(error->message, sizeof(error->message), message);
    }
    return status;
}

bool finite(float value) {
    return std::isfinite(value);
}

bool valid_color(const fission_skia_color_t& color) {
    const float values[] = {color.red, color.green, color.blue, color.alpha};
    for (float value : values) {
        if (!finite(value) || value < 0.0f || value > 1.0f) {
            return false;
        }
    }
    return true;
}

bool valid_rect(const fission_skia_rect_t& rect) {
    return finite(rect.x) && finite(rect.y) && finite(rect.width) &&
           finite(rect.height) && rect.width >= 0.0f && rect.height >= 0.0f;
}

bool valid_command_coordinates(const fission_skia_path_command_t& command) {
    const float values[] = {
        command.x1, command.y1, command.x2,
        command.y2, command.x3, command.y3,
    };
    for (float value : values) {
        if (!finite(value)) {
            return false;
        }
    }
    return true;
}

fission_skia_status_t validate_path(
    const fission_skia_frame_t& frame,
    const fission_skia_frame_op_t& operation,
    fission_skia_error_t* error) {
    if (operation.fill_rule != FISSION_SKIA_FILL_NON_ZERO &&
        operation.fill_rule != FISSION_SKIA_FILL_EVEN_ODD) {
        return fail(FISSION_SKIA_STATUS_INVALID_ARGUMENT, "execute_frame",
                    "fill path has an unknown fill rule", error);
    }
    const size_t offset = operation.path_offset;
    const size_t count = operation.path_count;
    if (count == 0 || offset > frame.path_command_count ||
        count > frame.path_command_count - offset) {
        return fail(FISSION_SKIA_STATUS_INVALID_ARGUMENT, "execute_frame",
                    "fill path command range is outside the frame", error);
    }
    bool has_current_point = false;
    for (size_t index = offset; index < offset + count; ++index) {
        const auto& command = frame.path_commands[index];
        if (command.struct_size != sizeof(fission_skia_path_command_t) ||
            !valid_command_coordinates(command)) {
            return fail(FISSION_SKIA_STATUS_INVALID_ARGUMENT, "execute_frame",
                        "path command has an incompatible layout or non-finite coordinate", error);
        }
        switch (command.verb) {
            case FISSION_SKIA_PATH_MOVE:
                has_current_point = true;
                break;
            case FISSION_SKIA_PATH_LINE:
            case FISSION_SKIA_PATH_QUAD:
            case FISSION_SKIA_PATH_CUBIC:
            case FISSION_SKIA_PATH_CLOSE:
                if (!has_current_point) {
                    return fail(FISSION_SKIA_STATUS_INVALID_ARGUMENT, "execute_frame",
                                "path contour must begin with move-to", error);
                }
                break;
            default:
                return fail(FISSION_SKIA_STATUS_INVALID_ARGUMENT, "execute_frame",
                            "path command has an unknown verb", error);
        }
    }
    return FISSION_SKIA_STATUS_OK;
}

fission_skia_status_t validate_frame(
    const fission_skia_frame_t* frame,
    fission_skia_error_t* error) {
    if (frame == nullptr || frame->struct_size != sizeof(fission_skia_frame_t)) {
        return fail(FISSION_SKIA_STATUS_INVALID_ARGUMENT, "execute_frame",
                    "frame is null or has an incompatible layout", error);
    }
    if ((frame->operation_count != 0 && frame->operations == nullptr) ||
        (frame->path_command_count != 0 && frame->path_commands == nullptr)) {
        return fail(FISSION_SKIA_STATUS_INVALID_ARGUMENT, "execute_frame",
                    "a non-empty frame array has a null pointer", error);
    }
    for (size_t index = 0; index < frame->operation_count; ++index) {
        const auto& operation = frame->operations[index];
        if (operation.struct_size != sizeof(fission_skia_frame_op_t) ||
            !valid_color(operation.color)) {
            return fail(FISSION_SKIA_STATUS_INVALID_ARGUMENT, "execute_frame",
                        "frame operation has an incompatible layout or invalid color", error);
        }
        switch (operation.kind) {
            case FISSION_SKIA_FRAME_CLEAR:
                break;
            case FISSION_SKIA_FRAME_FILL_RECT:
                if (!valid_rect(operation.rect)) {
                    return fail(FISSION_SKIA_STATUS_INVALID_ARGUMENT, "execute_frame",
                                "fill rectangle has invalid geometry", error);
                }
                break;
            case FISSION_SKIA_FRAME_FILL_PATH: {
                const auto status = validate_path(*frame, operation, error);
                if (status != FISSION_SKIA_STATUS_OK) {
                    return status;
                }
                break;
            }
            default:
                return fail(FISSION_SKIA_STATUS_UNSUPPORTED, "execute_frame",
                            "frame operation is not supported by ABI version 1", error);
        }
    }
    return FISSION_SKIA_STATUS_OK;
}

SkColor4f sk_color(const fission_skia_color_t& color) {
    return {color.red, color.green, color.blue, color.alpha};
}

SkPath sk_path(
    const fission_skia_path_command_t* commands,
    size_t count,
    uint32_t fill_rule) {
    SkPath path;
    path.setFillType(fill_rule == FISSION_SKIA_FILL_EVEN_ODD
        ? SkPathFillType::kEvenOdd
        : SkPathFillType::kWinding);
    for (size_t index = 0; index < count; ++index) {
        const auto& command = commands[index];
        switch (command.verb) {
            case FISSION_SKIA_PATH_MOVE:
                path.moveTo(command.x1, command.y1);
                break;
            case FISSION_SKIA_PATH_LINE:
                path.lineTo(command.x1, command.y1);
                break;
            case FISSION_SKIA_PATH_QUAD:
                path.quadTo(command.x1, command.y1, command.x2, command.y2);
                break;
            case FISSION_SKIA_PATH_CUBIC:
                path.cubicTo(command.x1, command.y1, command.x2, command.y2,
                             command.x3, command.y3);
                break;
            case FISSION_SKIA_PATH_CLOSE:
                path.close();
                break;
        }
    }
    return path;
}

template <typename State>
fission_skia_status_t check_owner(
    const State& state,
    const char* operation,
    fission_skia_error_t* error) {
    if (state.owner != std::this_thread::get_id()) {
        return fail(FISSION_SKIA_STATUS_WRONG_THREAD, operation,
                    "native handle was used from a thread other than its owner", error);
    }
    return FISSION_SKIA_STATUS_OK;
}

}  // namespace

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
    const auto handle = next_handle();
    registry().contexts.emplace(handle, std::move(state));
    parent->second->live_contexts += 1;
    *out_context = handle;
    clear_error(out_error);
    return FISSION_SKIA_STATUS_OK;
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
    state->surface = std::move(surface);
    const auto handle = next_handle();
    registry().surfaces.emplace(handle, std::move(state));
    parent->second->live_surfaces += 1;
    *out_surface = handle;
    clear_error(out_error);
    return FISSION_SKIA_STATUS_OK;
}

fission_skia_status_t fission_skia_surface_execute_frame(
    fission_skia_surface_handle_t surface,
    const fission_skia_frame_t* frame,
    fission_skia_error_t* out_error) {
    auto status = validate_frame(frame, out_error);
    if (status != FISSION_SKIA_STATUS_OK) return status;
    std::lock_guard<std::mutex> lock(registry().mutex);
    const auto found = registry().surfaces.find(surface);
    if (found == registry().surfaces.end()) {
        return fail(FISSION_SKIA_STATUS_INVALID_HANDLE, "execute_frame",
                    "surface handle is not live", out_error);
    }
    status = check_owner(*found->second, "execute_frame", out_error);
    if (status != FISSION_SKIA_STATUS_OK) return status;
    auto* canvas = found->second->surface->getCanvas();
    if (canvas == nullptr) {
        return fail(FISSION_SKIA_STATUS_SURFACE_LOST, "execute_frame",
                    "raster surface has no canvas", out_error);
    }
    auto srgb = SkColorSpace::MakeSRGB();
    for (size_t index = 0; index < frame->operation_count; ++index) {
        const auto& operation = frame->operations[index];
        if (operation.kind == FISSION_SKIA_FRAME_CLEAR) {
            // The destination surface is explicitly tagged sRGB, so clear's
            // color4f values use that destination color space.
            canvas->clear(sk_color(operation.color));
            continue;
        }
        SkPaint paint;
        paint.setAntiAlias(true);
        paint.setColor4f(sk_color(operation.color), srgb.get());
        paint.setStyle(SkPaint::kFill_Style);
        if (operation.kind == FISSION_SKIA_FRAME_FILL_RECT) {
            canvas->drawRect(
                SkRect::MakeXYWH(operation.rect.x, operation.rect.y,
                                 operation.rect.width, operation.rect.height),
                paint);
        } else {
            canvas->drawPath(
                sk_path(frame->path_commands + operation.path_offset,
                        operation.path_count, operation.fill_rule),
                paint);
        }
    }
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

}  // extern "C"
