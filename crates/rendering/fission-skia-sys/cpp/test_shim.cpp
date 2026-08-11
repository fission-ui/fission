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
    FISSION_SKIA_FEATURE_PAINT_STATE | FISSION_SKIA_FEATURE_PARAGRAPH |
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

bool valid_rect(const fission_skia_rect_t& rect) {
    return std::isfinite(rect.x) && std::isfinite(rect.y) &&
           std::isfinite(rect.width) && std::isfinite(rect.height) &&
           rect.width >= 0.0f && rect.height >= 0.0f;
}

bool valid_range(uint32_t offset, uint32_t count, size_t length) {
    const size_t start = offset;
    const size_t amount = count;
    return start <= length && amount <= length - start;
}

bool valid_paint(const fission_skia_frame_t& frame, const fission_skia_paint_t& paint) {
    if (paint.struct_size != sizeof(paint)) return false;
    if (paint.kind == FISSION_SKIA_PAINT_SOLID) return valid_color(paint.color);
    if (paint.kind != FISSION_SKIA_PAINT_LINEAR_GRADIENT &&
        paint.kind != FISSION_SKIA_PAINT_RADIAL_GRADIENT) return false;
    if (!std::isfinite(paint.start.x) || !std::isfinite(paint.start.y) ||
        !std::isfinite(paint.end.x) || !std::isfinite(paint.end.y) ||
        !std::isfinite(paint.radius) || paint.radius < 0.0f ||
        !valid_range(paint.stop_offset, paint.stop_count, frame.gradient_stop_count)) return false;
    float previous = 0.0f;
    for (uint32_t index = 0; index < paint.stop_count; ++index) {
        const auto& stop = frame.gradient_stops[paint.stop_offset + index];
        if (!std::isfinite(stop.offset) || stop.offset < 0.0f || stop.offset > 1.0f ||
            !valid_color(stop.color) || (index && stop.offset < previous)) return false;
        previous = stop.offset;
    }
    return true;
}

bool valid_stroke(const fission_skia_frame_t& frame, const fission_skia_stroke_t& stroke) {
    if (stroke.struct_size != sizeof(stroke) || !std::isfinite(stroke.width) ||
        stroke.width < 0.0f || stroke.line_cap < FISSION_SKIA_LINE_CAP_BUTT ||
        stroke.line_cap > FISSION_SKIA_LINE_CAP_SQUARE ||
        stroke.line_join < FISSION_SKIA_LINE_JOIN_MITER ||
        stroke.line_join > FISSION_SKIA_LINE_JOIN_BEVEL ||
        stroke.dash_count % 2 != 0 ||
        !valid_range(stroke.dash_offset, stroke.dash_count, frame.dash_interval_count)) return false;
    float sum = 0.0f;
    for (uint32_t index = 0; index < stroke.dash_count; ++index) {
        const float interval = frame.dash_intervals[stroke.dash_offset + index];
        if (!std::isfinite(interval) || interval < 0.0f) return false;
        sum += interval;
    }
    return stroke.dash_count == 0 || (std::isfinite(sum) && sum > 0.0f);
}

bool valid_path(const fission_skia_frame_t& frame, const fission_skia_frame_op_t& op) {
    if (op.fill_rule != FISSION_SKIA_FILL_NON_ZERO &&
        op.fill_rule != FISSION_SKIA_FILL_EVEN_ODD) return false;
    if (!op.path_count || !valid_range(op.path_offset, op.path_count,
                                       frame.path_command_count)) return false;
    bool current = false;
    for (uint32_t index = 0; index < op.path_count; ++index) {
        const auto& command = frame.path_commands[op.path_offset + index];
        if (command.struct_size != sizeof(command)) return false;
        const float values[] = {command.x1, command.y1, command.x2,
                                command.y2, command.x3, command.y3};
        for (float value : values) if (!std::isfinite(value)) return false;
        if (command.verb == FISSION_SKIA_PATH_MOVE) current = true;
        else if (!current || command.verb < FISSION_SKIA_PATH_LINE ||
                 command.verb > FISSION_SKIA_PATH_CLOSE) return false;
    }
    return true;
}

fission_skia_color_t representative_color(
    const fission_skia_frame_t& frame,
    const fission_skia_paint_t& paint) {
    if (paint.kind == FISSION_SKIA_PAINT_SOLID) return paint.color;
    if (paint.stop_count == 0) return {0.0f, 0.0f, 0.0f, 0.0f};
    return frame.gradient_stops[paint.stop_offset + paint.stop_count - 1].color;
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
        (frame->path_command_count && !frame->path_commands) ||
        (frame->gradient_stop_count && !frame->gradient_stops) ||
        (frame->dash_interval_count && !frame->dash_intervals))
        return fail(FISSION_SKIA_STATUS_INVALID_ARGUMENT, "execute_frame", "invalid frame", error);
    size_t save_depth = 0;
    for (size_t index = 0; index < frame->operation_count; ++index) {
        const auto& op = frame->operations[index];
        if (op.struct_size != sizeof(op))
            return fail(FISSION_SKIA_STATUS_INVALID_ARGUMENT, "execute_frame", "invalid operation", error);
        switch (op.kind) {
            case FISSION_SKIA_FRAME_CLEAR:
                if (!valid_paint(*frame, op.paint) ||
                    op.paint.kind != FISSION_SKIA_PAINT_SOLID)
                    return fail(FISSION_SKIA_STATUS_INVALID_ARGUMENT, "execute_frame",
                                "invalid clear paint", error);
                break;
            case FISSION_SKIA_FRAME_SAVE:
                save_depth += 1;
                break;
            case FISSION_SKIA_FRAME_RESTORE:
                if (!save_depth)
                    return fail(FISSION_SKIA_STATUS_INVALID_ARGUMENT, "execute_frame",
                                "restore without save", error);
                save_depth -= 1;
                break;
            case FISSION_SKIA_FRAME_CLIP_RECT:
            case FISSION_SKIA_FRAME_CLIP_ROUNDED_RECT:
                if (!valid_rect(op.rect) || !std::isfinite(op.radius) || op.radius < 0.0f)
                    return fail(FISSION_SKIA_STATUS_INVALID_ARGUMENT, "execute_frame",
                                "invalid clip", error);
                break;
            case FISSION_SKIA_FRAME_CONCAT_AFFINE: {
                const float values[] = {op.affine.scale_x, op.affine.skew_x,
                                        op.affine.translate_x, op.affine.skew_y,
                                        op.affine.scale_y, op.affine.translate_y};
                for (float value : values)
                    if (!std::isfinite(value))
                        return fail(FISSION_SKIA_STATUS_INVALID_ARGUMENT, "execute_frame",
                                    "invalid affine", error);
                break;
            }
            case FISSION_SKIA_FRAME_FILL_RECT:
                if (!valid_rect(op.rect) || !std::isfinite(op.radius) || op.radius < 0.0f ||
                    !valid_paint(*frame, op.paint))
                    return fail(FISSION_SKIA_STATUS_INVALID_ARGUMENT, "execute_frame",
                                "invalid fill rectangle", error);
                break;
            case FISSION_SKIA_FRAME_STROKE_RECT:
                if (!valid_rect(op.rect) || !std::isfinite(op.radius) || op.radius < 0.0f ||
                    !valid_paint(*frame, op.paint) || !valid_stroke(*frame, op.stroke))
                    return fail(FISSION_SKIA_STATUS_INVALID_ARGUMENT, "execute_frame",
                                "invalid stroke rectangle", error);
                break;
            case FISSION_SKIA_FRAME_FILL_PATH:
            case FISSION_SKIA_FRAME_STROKE_PATH:
                if (!valid_path(*frame, op) || !valid_paint(*frame, op.paint) ||
                    (op.kind == FISSION_SKIA_FRAME_STROKE_PATH &&
                     !valid_stroke(*frame, op.stroke)))
                    return fail(FISSION_SKIA_STATUS_INVALID_ARGUMENT, "execute_frame",
                                "invalid path paint", error);
                break;
            case FISSION_SKIA_FRAME_BOX_SHADOW:
                if (!valid_rect(op.rect) || !std::isfinite(op.radius) || op.radius < 0.0f ||
                    op.shadow.struct_size != sizeof(op.shadow) || op.shadow.inset > 1 ||
                    !valid_color(op.shadow.color) || !std::isfinite(op.shadow.blur_radius) ||
                    op.shadow.blur_radius < 0.0f || !std::isfinite(op.shadow.spread_radius) ||
                    !std::isfinite(op.shadow.offset_x) || !std::isfinite(op.shadow.offset_y))
                    return fail(FISSION_SKIA_STATUS_INVALID_ARGUMENT, "execute_frame",
                                "invalid box shadow", error);
                break;
            default:
                return fail(FISSION_SKIA_STATUS_UNSUPPORTED, "execute_frame",
                            "unknown operation", error);
        }
    }
    if (save_depth)
        return fail(FISSION_SKIA_STATUS_INVALID_ARGUMENT, "execute_frame",
                    "unrestored save", error);
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
                                      static_cast<float>(found->second.height)}, op.paint.color);
        } else if (op.kind == FISSION_SKIA_FRAME_FILL_RECT) {
            paint_rect(found->second, op.rect, representative_color(*frame, op.paint));
        }
        // State, gradients, strokes, paths, and shadows are intentionally
        // validation-only in the ABI ownership test double.
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
