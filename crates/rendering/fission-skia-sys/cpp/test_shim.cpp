#define FISSION_SKIA_TEST_SHIM 1
#include "fission_skia.h"

#include <algorithm>
#include <atomic>
#include <cmath>
#include <cstring>
#include <mutex>
#include <thread>
#include <unordered_map>
#include <vector>

#ifndef FISSION_SKIA_REVISION
#define FISSION_SKIA_REVISION "unknown"
#endif

namespace {

constexpr uint64_t kFeatures =
    FISSION_SKIA_FEATURE_RASTER_SURFACE | FISSION_SKIA_FEATURE_BASIC_FRAME |
    FISSION_SKIA_FEATURE_RGBA_READBACK | FISSION_SKIA_FEATURE_STRUCTURED_ERRORS |
    FISSION_SKIA_FEATURE_THREAD_AFFINITY | FISSION_SKIA_FEATURE_MEMORY_PRESSURE |
    FISSION_SKIA_FEATURE_TEST_SHIM;

struct Engine { std::thread::id owner; uint64_t children = 0; };
struct Context { std::thread::id owner; uint64_t engine = 0; uint64_t children = 0; };
struct Surface {
    std::thread::id owner;
    uint64_t context = 0;
    uint32_t width = 0;
    uint32_t height = 0;
    std::vector<uint8_t> pixels;
};
struct State {
    std::mutex mutex;
    std::unordered_map<uint64_t, Engine> engines;
    std::unordered_map<uint64_t, Context> contexts;
    std::unordered_map<uint64_t, Surface> surfaces;
    std::atomic<uint64_t> next{1};
    std::atomic<uint64_t> errors{1};
};

State& state() { static State value; return value; }
uint64_t handle() { return state().next.fetch_add(1, std::memory_order_relaxed); }

void text(char* destination, size_t capacity, const char* source) {
    if (!capacity) return;
    const auto copied = std::min(capacity - 1, std::strlen(source));
    std::memcpy(destination, source, copied);
    destination[copied] = '\0';
}

void clear(fission_skia_error_t* error) {
    if (error && error->struct_size == sizeof(*error)) {
        error->code = 0;
        error->sequence = 0;
        error->operation[0] = '\0';
        error->message[0] = '\0';
    }
}

fission_skia_status_t fail(fission_skia_status_t status, const char* operation,
                           const char* message, fission_skia_error_t* error) {
    if (error && error->struct_size == sizeof(*error)) {
        error->code = status;
        error->sequence = state().errors.fetch_add(1, std::memory_order_relaxed);
        text(error->operation, sizeof(error->operation), operation);
        text(error->message, sizeof(error->message), message);
    }
    return status;
}

template <typename T>
fission_skia_status_t owner(const T& value, const char* operation,
                            fission_skia_error_t* error) {
    return value.owner == std::this_thread::get_id()
        ? FISSION_SKIA_STATUS_OK
        : fail(FISSION_SKIA_STATUS_WRONG_THREAD, operation,
               "test handle used from a non-owner thread", error);
}

bool valid_color(const fission_skia_color_t& color) {
    const float values[] = {color.red, color.green, color.blue, color.alpha};
    for (float value : values) {
        if (!std::isfinite(value) || value < 0.0f || value > 1.0f) return false;
    }
    return true;
}

uint8_t channel(float value) {
    return static_cast<uint8_t>(std::lround(value * 255.0f));
}

void paint_rect(Surface& surface, const fission_skia_rect_t& rect,
                const fission_skia_color_t& color) {
    const int left = std::max(0, static_cast<int>(std::floor(rect.x)));
    const int top = std::max(0, static_cast<int>(std::floor(rect.y)));
    const int right = std::min(static_cast<int>(surface.width),
                               static_cast<int>(std::ceil(rect.x + rect.width)));
    const int bottom = std::min(static_cast<int>(surface.height),
                                static_cast<int>(std::ceil(rect.y + rect.height)));
    for (int y = top; y < bottom; ++y) {
        for (int x = left; x < right; ++x) {
            const size_t offset = (static_cast<size_t>(y) * surface.width + x) * 4;
            surface.pixels[offset] = channel(color.red);
            surface.pixels[offset + 1] = channel(color.green);
            surface.pixels[offset + 2] = channel(color.blue);
            surface.pixels[offset + 3] = channel(color.alpha);
        }
    }
}

}  // namespace

extern "C" {

fission_skia_status_t fission_skia_get_abi_info(
    fission_skia_abi_info_t* info, fission_skia_error_t* error) {
    if (!info || info->struct_size != sizeof(*info))
        return fail(FISSION_SKIA_STATUS_INVALID_ARGUMENT, "get_abi_info",
                    "invalid info output", error);
    info->abi_version = FISSION_SKIA_ABI_VERSION;
    info->feature_bits = kFeatures;
    text(info->skia_revision, sizeof(info->skia_revision), FISSION_SKIA_REVISION);
    text(info->build_profile, sizeof(info->build_profile), "test-shim");
    clear(error);
    return FISSION_SKIA_STATUS_OK;
}

fission_skia_status_t fission_skia_engine_create(
    const fission_skia_engine_config_t* config, fission_skia_engine_handle_t* output,
    fission_skia_error_t* error) {
    if (!config || config->struct_size != sizeof(*config) || !output)
        return fail(FISSION_SKIA_STATUS_INVALID_ARGUMENT, "engine_create",
                    "invalid engine configuration", error);
    *output = 0;
    if (config->expected_abi_version != FISSION_SKIA_ABI_VERSION)
        return fail(FISSION_SKIA_STATUS_ABI_MISMATCH, "engine_create",
                    "ABI mismatch", error);
    if (config->required_feature_bits & ~kFeatures)
        return fail(FISSION_SKIA_STATUS_UNSUPPORTED, "engine_create",
                    "required feature is absent", error);
    const auto id = handle();
    std::lock_guard<std::mutex> lock(state().mutex);
    state().engines.emplace(id, Engine{std::this_thread::get_id(), 0});
    *output = id;
    clear(error);
    return FISSION_SKIA_STATUS_OK;
}

fission_skia_status_t fission_skia_engine_destroy(
    fission_skia_engine_handle_t id, fission_skia_error_t* error) {
    std::lock_guard<std::mutex> lock(state().mutex);
    auto found = state().engines.find(id);
    if (found == state().engines.end())
        return fail(FISSION_SKIA_STATUS_INVALID_HANDLE, "engine_destroy", "invalid engine", error);
    auto status = owner(found->second, "engine_destroy", error);
    if (status) return status;
    if (found->second.children)
        return fail(FISSION_SKIA_STATUS_INVALID_STATE, "engine_destroy", "live contexts", error);
    state().engines.erase(found);
    clear(error);
    return FISSION_SKIA_STATUS_OK;
}

fission_skia_status_t fission_skia_context_create_raster(
    fission_skia_engine_handle_t engine, fission_skia_context_handle_t* output,
    fission_skia_error_t* error) {
    if (!output) return fail(FISSION_SKIA_STATUS_INVALID_ARGUMENT, "context_create_raster",
                             "null output", error);
    *output = 0;
    std::lock_guard<std::mutex> lock(state().mutex);
    auto parent = state().engines.find(engine);
    if (parent == state().engines.end())
        return fail(FISSION_SKIA_STATUS_INVALID_HANDLE, "context_create_raster",
                    "invalid engine", error);
    auto status = owner(parent->second, "context_create_raster", error);
    if (status) return status;
    const auto id = handle();
    state().contexts.emplace(id, Context{std::this_thread::get_id(), engine, 0});
    parent->second.children += 1;
    *output = id;
    clear(error);
    return FISSION_SKIA_STATUS_OK;
}

fission_skia_status_t fission_skia_context_trim_memory(
    fission_skia_context_handle_t id, uint32_t pressure, fission_skia_error_t* error) {
    std::lock_guard<std::mutex> lock(state().mutex);
    auto found = state().contexts.find(id);
    if (found == state().contexts.end())
        return fail(FISSION_SKIA_STATUS_INVALID_HANDLE, "context_trim_memory", "invalid context", error);
    auto status = owner(found->second, "context_trim_memory", error);
    if (status) return status;
    if (pressure != FISSION_SKIA_MEMORY_PRESSURE_MODERATE &&
        pressure != FISSION_SKIA_MEMORY_PRESSURE_CRITICAL)
        return fail(FISSION_SKIA_STATUS_INVALID_ARGUMENT, "context_trim_memory",
                    "invalid pressure", error);
    clear(error);
    return FISSION_SKIA_STATUS_OK;
}

fission_skia_status_t fission_skia_context_destroy(
    fission_skia_context_handle_t id, fission_skia_error_t* error) {
    std::lock_guard<std::mutex> lock(state().mutex);
    auto found = state().contexts.find(id);
    if (found == state().contexts.end())
        return fail(FISSION_SKIA_STATUS_INVALID_HANDLE, "context_destroy", "invalid context", error);
    auto status = owner(found->second, "context_destroy", error);
    if (status) return status;
    if (found->second.children)
        return fail(FISSION_SKIA_STATUS_INVALID_STATE, "context_destroy", "live surfaces", error);
    const auto engine = found->second.engine;
    state().contexts.erase(found);
    state().engines.at(engine).children -= 1;
    clear(error);
    return FISSION_SKIA_STATUS_OK;
}

fission_skia_status_t fission_skia_surface_create_raster(
    fission_skia_context_handle_t context, uint32_t width, uint32_t height,
    fission_skia_surface_handle_t* output, fission_skia_error_t* error) {
    if (!output || !width || !height)
        return fail(FISSION_SKIA_STATUS_INVALID_ARGUMENT, "surface_create_raster",
                    "invalid output or dimensions", error);
    *output = 0;
    const size_t pixels = static_cast<size_t>(width) * height;
    if (pixels > static_cast<size_t>(-1) / 4)
        return fail(FISSION_SKIA_STATUS_OUT_OF_MEMORY, "surface_create_raster",
                    "surface length overflow", error);
    std::lock_guard<std::mutex> lock(state().mutex);
    auto parent = state().contexts.find(context);
    if (parent == state().contexts.end())
        return fail(FISSION_SKIA_STATUS_INVALID_HANDLE, "surface_create_raster",
                    "invalid context", error);
    auto status = owner(parent->second, "surface_create_raster", error);
    if (status) return status;
    const auto id = handle();
    state().surfaces.emplace(id, Surface{std::this_thread::get_id(), context, width, height,
                                        std::vector<uint8_t>(pixels * 4, 0)});
    parent->second.children += 1;
    *output = id;
    clear(error);
    return FISSION_SKIA_STATUS_OK;
}

fission_skia_status_t fission_skia_surface_execute_frame(
    fission_skia_surface_handle_t id, const fission_skia_frame_t* frame,
    fission_skia_error_t* error) {
    if (!frame || frame->struct_size != sizeof(*frame) ||
        (frame->operation_count && !frame->operations) ||
        (frame->path_command_count && !frame->path_commands))
        return fail(FISSION_SKIA_STATUS_INVALID_ARGUMENT, "execute_frame", "invalid frame", error);
    for (size_t index = 0; index < frame->operation_count; ++index) {
        const auto& op = frame->operations[index];
        if (op.struct_size != sizeof(op) || !valid_color(op.color))
            return fail(FISSION_SKIA_STATUS_INVALID_ARGUMENT, "execute_frame", "invalid operation", error);
        if (op.kind == FISSION_SKIA_FRAME_FILL_RECT &&
            (!std::isfinite(op.rect.x) || !std::isfinite(op.rect.y) ||
             !std::isfinite(op.rect.width) || !std::isfinite(op.rect.height) ||
             op.rect.width < 0 || op.rect.height < 0))
            return fail(FISSION_SKIA_STATUS_INVALID_ARGUMENT, "execute_frame", "invalid rect", error);
        if (op.kind == FISSION_SKIA_FRAME_FILL_PATH &&
            (!op.path_count || op.path_offset > frame->path_command_count ||
             op.path_count > frame->path_command_count - op.path_offset))
            return fail(FISSION_SKIA_STATUS_INVALID_ARGUMENT, "execute_frame", "invalid path range", error);
        if (op.kind < FISSION_SKIA_FRAME_CLEAR || op.kind > FISSION_SKIA_FRAME_FILL_PATH)
            return fail(FISSION_SKIA_STATUS_UNSUPPORTED, "execute_frame", "unknown operation", error);
    }
    std::lock_guard<std::mutex> lock(state().mutex);
    auto found = state().surfaces.find(id);
    if (found == state().surfaces.end())
        return fail(FISSION_SKIA_STATUS_INVALID_HANDLE, "execute_frame", "invalid surface", error);
    auto status = owner(found->second, "execute_frame", error);
    if (status) return status;
    for (size_t index = 0; index < frame->operation_count; ++index) {
        const auto& op = frame->operations[index];
        if (op.kind == FISSION_SKIA_FRAME_CLEAR) {
            paint_rect(found->second, {0, 0, static_cast<float>(found->second.width),
                                      static_cast<float>(found->second.height)}, op.color);
        } else if (op.kind == FISSION_SKIA_FRAME_FILL_RECT) {
            paint_rect(found->second, op.rect, op.color);
        }
        // Paths are intentionally validation-only in the ABI test double.
    }
    clear(error);
    return FISSION_SKIA_STATUS_OK;
}

fission_skia_status_t fission_skia_surface_read_pixels_rgba8888(
    fission_skia_surface_handle_t id, const fission_skia_pixel_rect_t* rect,
    uint8_t* destination, size_t length, size_t row_bytes, size_t* required,
    fission_skia_error_t* error) {
    if (!rect || !required || rect->x < 0 || rect->y < 0 || !rect->width || !rect->height)
        return fail(FISSION_SKIA_STATUS_INVALID_ARGUMENT, "read_pixels_rgba8888",
                    "invalid readback arguments", error);
    const size_t tight = static_cast<size_t>(rect->width) * 4;
    if (row_bytes < tight || rect->height > static_cast<size_t>(-1) / row_bytes)
        return fail(FISSION_SKIA_STATUS_INVALID_ARGUMENT, "read_pixels_rgba8888",
                    "invalid row stride", error);
    *required = row_bytes * rect->height;
    if (!destination || length < *required)
        return fail(FISSION_SKIA_STATUS_INVALID_ARGUMENT, "read_pixels_rgba8888",
                    "destination is too small", error);
    std::lock_guard<std::mutex> lock(state().mutex);
    auto found = state().surfaces.find(id);
    if (found == state().surfaces.end())
        return fail(FISSION_SKIA_STATUS_INVALID_HANDLE, "read_pixels_rgba8888", "invalid surface", error);
    auto status = owner(found->second, "read_pixels_rgba8888", error);
    if (status) return status;
    if (static_cast<uint64_t>(rect->x) + rect->width > found->second.width ||
        static_cast<uint64_t>(rect->y) + rect->height > found->second.height)
        return fail(FISSION_SKIA_STATUS_INVALID_ARGUMENT, "read_pixels_rgba8888",
                    "rectangle outside surface", error);
    for (uint32_t row = 0; row < rect->height; ++row) {
        const size_t source = ((static_cast<size_t>(rect->y) + row) * found->second.width + rect->x) * 4;
        std::memcpy(destination + row * row_bytes, found->second.pixels.data() + source, tight);
    }
    clear(error);
    return FISSION_SKIA_STATUS_OK;
}

fission_skia_status_t fission_skia_surface_destroy(
    fission_skia_surface_handle_t id, fission_skia_error_t* error) {
    std::lock_guard<std::mutex> lock(state().mutex);
    auto found = state().surfaces.find(id);
    if (found == state().surfaces.end())
        return fail(FISSION_SKIA_STATUS_INVALID_HANDLE, "surface_destroy", "invalid surface", error);
    auto status = owner(found->second, "surface_destroy", error);
    if (status) return status;
    const auto context = found->second.context;
    state().surfaces.erase(found);
    state().contexts.at(context).children -= 1;
    clear(error);
    return FISSION_SKIA_STATUS_OK;
}

fission_skia_status_t fission_skia_test_live_counts(
    fission_skia_test_counts_t* counts, fission_skia_error_t* error) {
    if (!counts) return fail(FISSION_SKIA_STATUS_INVALID_ARGUMENT, "test_live_counts",
                             "null counts", error);
    std::lock_guard<std::mutex> lock(state().mutex);
    counts->engines = state().engines.size();
    counts->contexts = state().contexts.size();
    counts->surfaces = state().surfaces.size();
    clear(error);
    return FISSION_SKIA_STATUS_OK;
}

}  // extern "C"
