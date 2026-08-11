#define FISSION_SKIA_TEST_SHIM 1
#include "fission_skia.h"
#include "fission_skia_paragraph_internal.h"

#include <algorithm>
#include <atomic>
#include <cmath>
#include <cstring>
#include <mutex>
#include <thread>
#include <unordered_map>
#include <utility>
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
    FISSION_SKIA_FEATURE_OPACITY_LAYER |
    FISSION_SKIA_FEATURE_IMAGE_DECODE |
    FISSION_SKIA_FEATURE_BACKDROP_BLUR |
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
struct DecodedImage {
    uint32_t width = 0;
    uint32_t height = 0;
    std::vector<uint8_t> pixels;
};
struct PixelTarget {
    uint32_t width = 0;
    uint32_t height = 0;
    std::vector<uint8_t>* pixels = nullptr;
    fission_skia_rect_t clip = {};
};
struct OpacityLayer {
    fission_skia_rect_t bounds = {};
    float alpha = 1.0f;
    std::vector<uint8_t> pixels;
};
enum class SavedKind { kCanvasState, kOpacityLayer };
struct State {
    std::mutex mutex;
    std::unordered_map<uint64_t, Engine> engines;
    std::unordered_map<uint64_t, Context> contexts;
    std::unordered_map<uint64_t, Surface> surfaces;
    std::unordered_map<uint64_t, DecodedImage> images;
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

bool valid_non_empty_rect(const fission_skia_rect_t& rect) {
    return valid_rect(rect) && rect.width > 0.0f && rect.height > 0.0f;
}

uint32_t read_u32_le(const uint8_t* bytes) {
    return static_cast<uint32_t>(bytes[0]) |
           (static_cast<uint32_t>(bytes[1]) << 8) |
           (static_cast<uint32_t>(bytes[2]) << 16) |
           (static_cast<uint32_t>(bytes[3]) << 24);
}

void write_image_info(
    const DecodedImage& image,
    fission_skia_image_info_t* info) {
    info->width = image.width;
    info->height = image.height;
    info->reserved = 0;
    info->approximate_decoded_bytes = image.pixels.size();
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

fission_skia_status_t validate_image_draw(
    const fission_skia_image_draw_t& draw,
    fission_skia_error_t* error) {
    if (draw.struct_size != sizeof(draw) ||
        (draw.sampling != FISSION_SKIA_IMAGE_SAMPLING_NEAREST &&
         draw.sampling != FISSION_SKIA_IMAGE_SAMPLING_LINEAR) ||
        !valid_non_empty_rect(draw.source) ||
        !valid_non_empty_rect(draw.destination) ||
        draw.source.x < 0.0f || draw.source.y < 0.0f) {
        return fail(FISSION_SKIA_STATUS_INVALID_ARGUMENT, "execute_frame",
                    "invalid image draw", error);
    }
    std::lock_guard<std::mutex> lock(state().mutex);
    const auto found = state().images.find(draw.image);
    if (found == state().images.end()) {
        return fail(FISSION_SKIA_STATUS_INVALID_HANDLE, "execute_frame",
                    "invalid image handle", error);
    }
    const double right = static_cast<double>(draw.source.x) + draw.source.width;
    const double bottom = static_cast<double>(draw.source.y) + draw.source.height;
    if (right > found->second.width || bottom > found->second.height) {
        return fail(FISSION_SKIA_STATUS_INVALID_ARGUMENT, "execute_frame",
                    "image source rectangle is outside the image", error);
    }
    return FISSION_SKIA_STATUS_OK;
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

struct PixelBounds {
    int left = 0;
    int top = 0;
    int right = 0;
    int bottom = 0;
};

PixelBounds intersected_pixel_bounds(
    const PixelTarget& target,
    const fission_skia_rect_t& rect) {
    const double left = std::max<double>(rect.x, target.clip.x);
    const double top = std::max<double>(rect.y, target.clip.y);
    const double right = std::min<double>(
        static_cast<double>(rect.x) + rect.width,
        static_cast<double>(target.clip.x) + target.clip.width);
    const double bottom = std::min<double>(
        static_cast<double>(rect.y) + rect.height,
        static_cast<double>(target.clip.y) + target.clip.height);
    if (right <= left || bottom <= top) return {};
    return {
        static_cast<int>(std::clamp(std::floor(left), 0.0,
                                    static_cast<double>(target.width))),
        static_cast<int>(std::clamp(std::floor(top), 0.0,
                                    static_cast<double>(target.height))),
        static_cast<int>(std::clamp(std::ceil(right), 0.0,
                                    static_cast<double>(target.width))),
        static_cast<int>(std::clamp(std::ceil(bottom), 0.0,
                                    static_cast<double>(target.height))),
    };
}

void replace_pixel(uint8_t* destination, const fission_skia_color_t& color) {
    destination[0] = channel(color.red);
    destination[1] = channel(color.green);
    destination[2] = channel(color.blue);
    destination[3] = channel(color.alpha);
}

void blend_pixel(
    uint8_t* destination,
    const fission_skia_color_t& source,
    float opacity = 1.0f) {
    const float source_alpha = source.alpha * opacity;
    const float destination_alpha = static_cast<float>(destination[3]) / 255.0f;
    const float output_alpha = source_alpha + destination_alpha * (1.0f - source_alpha);
    if (output_alpha <= 0.0f) {
        std::memset(destination, 0, 4);
        return;
    }
    const float destination_weight = destination_alpha * (1.0f - source_alpha);
    const float destination_red = static_cast<float>(destination[0]) / 255.0f;
    const float destination_green = static_cast<float>(destination[1]) / 255.0f;
    const float destination_blue = static_cast<float>(destination[2]) / 255.0f;
    destination[0] = channel(
        (source.red * source_alpha + destination_red * destination_weight) / output_alpha);
    destination[1] = channel(
        (source.green * source_alpha + destination_green * destination_weight) / output_alpha);
    destination[2] = channel(
        (source.blue * source_alpha + destination_blue * destination_weight) / output_alpha);
    destination[3] = channel(output_alpha);
}

void paint_rect(
    PixelTarget& target,
    const fission_skia_rect_t& rect,
    const fission_skia_color_t& color,
    bool replace = false) {
    const PixelBounds bounds = intersected_pixel_bounds(target, rect);
    if (target.pixels == nullptr) return;
    auto& pixels = *target.pixels;
    const int left = bounds.left;
    const int top = bounds.top;
    const int right = bounds.right;
    const int bottom = bounds.bottom;
    for (int y = top; y < bottom; ++y) {
        for (int x = left; x < right; ++x) {
            const size_t offset = (static_cast<size_t>(y) * target.width + x) * 4;
            if (replace) replace_pixel(pixels.data() + offset, color);
            else blend_pixel(pixels.data() + offset, color);
        }
    }
}

PixelTarget current_target(Surface& surface, std::vector<OpacityLayer>& layers) {
    if (!layers.empty()) {
        auto& layer = layers.back();
        return {surface.width, surface.height, &layer.pixels, layer.bounds};
    }
    return {
        surface.width,
        surface.height,
        &surface.pixels,
        {0.0f, 0.0f, static_cast<float>(surface.width), static_cast<float>(surface.height)},
    };
}

void composite_layer(PixelTarget& destination, const OpacityLayer& layer) {
    const PixelBounds bounds = intersected_pixel_bounds(destination, layer.bounds);
    if (destination.pixels == nullptr) return;
    auto& pixels = *destination.pixels;
    for (int y = bounds.top; y < bounds.bottom; ++y) {
        for (int x = bounds.left; x < bounds.right; ++x) {
            const size_t offset = (static_cast<size_t>(y) * destination.width + x) * 4;
            const fission_skia_color_t source = {
                static_cast<float>(layer.pixels[offset]) / 255.0f,
                static_cast<float>(layer.pixels[offset + 1]) / 255.0f,
                static_cast<float>(layer.pixels[offset + 2]) / 255.0f,
                static_cast<float>(layer.pixels[offset + 3]) / 255.0f,
            };
            blend_pixel(pixels.data() + offset, source, layer.alpha);
        }
    }
}

bool rounded_rect_contains(
    const fission_skia_rect_t& rect,
    float radius,
    float x,
    float y) {
    const float right = rect.x + rect.width;
    const float bottom = rect.y + rect.height;
    if (x < rect.x || x >= right || y < rect.y || y >= bottom) return false;
    const float resolved_radius = std::min(
        radius, std::min(rect.width, rect.height) * 0.5f);
    if (resolved_radius <= 0.0f) return true;
    if ((x >= rect.x + resolved_radius && x < right - resolved_radius) ||
        (y >= rect.y + resolved_radius && y < bottom - resolved_radius)) {
        return true;
    }
    const float center_x = x < rect.x + resolved_radius
        ? rect.x + resolved_radius
        : right - resolved_radius;
    const float center_y = y < rect.y + resolved_radius
        ? rect.y + resolved_radius
        : bottom - resolved_radius;
    const float dx = x - center_x;
    const float dy = y - center_y;
    return dx * dx + dy * dy <= resolved_radius * resolved_radius;
}

void blur_backdrop(
    PixelTarget& target,
    const fission_skia_rect_t& rect,
    float radius,
    float sigma) {
    if (target.pixels == nullptr || sigma == 0.0f ||
        rect.width == 0.0f || rect.height == 0.0f) {
        return;
    }

    // Skia caps mapped blur sigma at 532 physical pixels. Mirroring that cap
    // keeps the deterministic ABI double bounded for hostile-but-finite input.
    const double effective_sigma = std::min<double>(sigma, 532.0);
    const int kernel_radius = static_cast<int>(std::ceil(effective_sigma * 3.0));
    std::vector<double> kernel(static_cast<size_t>(kernel_radius) * 2 + 1);
    double weight_sum = 0.0;
    const double denominator = 2.0 * effective_sigma * effective_sigma;
    for (int offset = -kernel_radius; offset <= kernel_radius; ++offset) {
        const double weight = std::exp(-(static_cast<double>(offset) * offset) / denominator);
        kernel[static_cast<size_t>(offset + kernel_radius)] = weight;
        weight_sum += weight;
    }
    for (double& weight : kernel) weight /= weight_sum;

    const size_t pixel_count = static_cast<size_t>(target.width) * target.height;
    const auto source = *target.pixels;
    std::vector<double> horizontal(pixel_count * 4, 0.0);
    std::vector<uint8_t> blurred(pixel_count * 4, 0);
    for (uint32_t y = 0; y < target.height; ++y) {
        for (uint32_t x = 0; x < target.width; ++x) {
            const size_t destination = (static_cast<size_t>(y) * target.width + x) * 4;
            for (int offset = -kernel_radius; offset <= kernel_radius; ++offset) {
                const int sample_x = std::clamp(
                    static_cast<int>(x) + offset, 0, static_cast<int>(target.width) - 1);
                const size_t sample =
                    (static_cast<size_t>(y) * target.width + sample_x) * 4;
                const double weight = kernel[static_cast<size_t>(offset + kernel_radius)];
                for (size_t channel_index = 0; channel_index < 4; ++channel_index) {
                    horizontal[destination + channel_index] +=
                        static_cast<double>(source[sample + channel_index]) * weight;
                }
            }
        }
    }
    for (uint32_t y = 0; y < target.height; ++y) {
        for (uint32_t x = 0; x < target.width; ++x) {
            const size_t destination = (static_cast<size_t>(y) * target.width + x) * 4;
            double channels[4] = {};
            for (int offset = -kernel_radius; offset <= kernel_radius; ++offset) {
                const int sample_y = std::clamp(
                    static_cast<int>(y) + offset, 0, static_cast<int>(target.height) - 1);
                const size_t sample =
                    (static_cast<size_t>(sample_y) * target.width + x) * 4;
                const double weight = kernel[static_cast<size_t>(offset + kernel_radius)];
                for (size_t channel_index = 0; channel_index < 4; ++channel_index) {
                    channels[channel_index] += horizontal[sample + channel_index] * weight;
                }
            }
            for (size_t channel_index = 0; channel_index < 4; ++channel_index) {
                blurred[destination + channel_index] = static_cast<uint8_t>(
                    std::clamp(std::lround(channels[channel_index]), 0L, 255L));
            }
        }
    }

    const PixelBounds bounds = intersected_pixel_bounds(target, rect);
    auto& destination = *target.pixels;
    for (int y = bounds.top; y < bounds.bottom; ++y) {
        for (int x = bounds.left; x < bounds.right; ++x) {
            if (!rounded_rect_contains(rect, radius,
                                       static_cast<float>(x) + 0.5f,
                                       static_cast<float>(y) + 0.5f)) {
                continue;
            }
            const size_t pixel = (static_cast<size_t>(y) * target.width + x) * 4;
            std::memcpy(destination.data() + pixel, blurred.data() + pixel, 4);
        }
    }
}

fission_skia_color_t image_pixel(const DecodedImage& image, int x, int y) {
    x = std::clamp(x, 0, static_cast<int>(image.width) - 1);
    y = std::clamp(y, 0, static_cast<int>(image.height) - 1);
    const size_t offset = (static_cast<size_t>(y) * image.width + x) * 4;
    return {
        static_cast<float>(image.pixels[offset]) / 255.0f,
        static_cast<float>(image.pixels[offset + 1]) / 255.0f,
        static_cast<float>(image.pixels[offset + 2]) / 255.0f,
        static_cast<float>(image.pixels[offset + 3]) / 255.0f,
    };
}

fission_skia_color_t interpolate_color(
    const fission_skia_color_t& first,
    const fission_skia_color_t& second,
    float amount) {
    return {
        first.red + (second.red - first.red) * amount,
        first.green + (second.green - first.green) * amount,
        first.blue + (second.blue - first.blue) * amount,
        first.alpha + (second.alpha - first.alpha) * amount,
    };
}

fission_skia_color_t sample_image(
    const DecodedImage& image,
    const fission_skia_image_draw_t& draw,
    float destination_x,
    float destination_y) {
    const float source_x = draw.source.x +
        ((destination_x - draw.destination.x) / draw.destination.width) * draw.source.width;
    const float source_y = draw.source.y +
        ((destination_y - draw.destination.y) / draw.destination.height) * draw.source.height;
    const int min_x = static_cast<int>(std::floor(draw.source.x));
    const int min_y = static_cast<int>(std::floor(draw.source.y));
    const int max_x = static_cast<int>(std::ceil(draw.source.x + draw.source.width)) - 1;
    const int max_y = static_cast<int>(std::ceil(draw.source.y + draw.source.height)) - 1;
    if (draw.sampling == FISSION_SKIA_IMAGE_SAMPLING_NEAREST) {
        return image_pixel(image, std::clamp(static_cast<int>(std::floor(source_x)), min_x, max_x),
                           std::clamp(static_cast<int>(std::floor(source_y)), min_y, max_y));
    }

    const float center_x = source_x - 0.5f;
    const float center_y = source_y - 0.5f;
    const int raw_x0 = static_cast<int>(std::floor(center_x));
    const int raw_y0 = static_cast<int>(std::floor(center_y));
    const float x_amount = center_x - raw_x0;
    const float y_amount = center_y - raw_y0;
    const int x0 = std::clamp(raw_x0, min_x, max_x);
    const int y0 = std::clamp(raw_y0, min_y, max_y);
    const int x1 = std::clamp(raw_x0 + 1, min_x, max_x);
    const int y1 = std::clamp(raw_y0 + 1, min_y, max_y);
    const auto top = interpolate_color(image_pixel(image, x0, y0),
                                       image_pixel(image, x1, y0), x_amount);
    const auto bottom = interpolate_color(image_pixel(image, x0, y1),
                                          image_pixel(image, x1, y1), x_amount);
    return interpolate_color(top, bottom, y_amount);
}

void draw_image(
    PixelTarget& target,
    const DecodedImage& image,
    const fission_skia_image_draw_t& draw) {
    if (target.pixels == nullptr) return;
    const PixelBounds bounds = intersected_pixel_bounds(target, draw.destination);
    auto& pixels = *target.pixels;
    for (int y = bounds.top; y < bounds.bottom; ++y) {
        const float center_y = static_cast<float>(y) + 0.5f;
        if (center_y < draw.destination.y ||
            center_y >= draw.destination.y + draw.destination.height) continue;
        for (int x = bounds.left; x < bounds.right; ++x) {
            const float center_x = static_cast<float>(x) + 0.5f;
            if (center_x < draw.destination.x ||
                center_x >= draw.destination.x + draw.destination.width) continue;
            const size_t offset = (static_cast<size_t>(y) * target.width + x) * 4;
            blend_pixel(pixels.data() + offset,
                        sample_image(image, draw, center_x, center_y));
        }
    }
}

void paint_paragraph_rect(void* context, const fission_skia_paragraph_rect_t& rect,
                          const fission_skia_color_t& color) {
    auto& target = *static_cast<PixelTarget*>(context);
    paint_rect(target, {rect.x, rect.y, rect.width, rect.height}, color);
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
            case FISSION_SKIA_FRAME_OPACITY_LAYER:
                if (!valid_rect(op.rect) || !std::isfinite(op.opacity) ||
                    op.opacity < 0.0f || op.opacity > 1.0f)
                    return fail(FISSION_SKIA_STATUS_INVALID_ARGUMENT, "execute_frame",
                                "invalid opacity layer", error);
                save_depth += 1;
                break;
            case FISSION_SKIA_FRAME_RESTORE:
                if (!save_depth)
                    return fail(FISSION_SKIA_STATUS_INVALID_ARGUMENT, "execute_frame",
                                "restore without save or opacity layer", error);
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
            case FISSION_SKIA_FRAME_DRAW_PARAGRAPH: {
                if (!std::isfinite(op.rect.x) || !std::isfinite(op.rect.y) ||
                    op.rect.width != 0.0f || op.rect.height != 0.0f ||
                    !std::isfinite(op.radius) || op.radius <= 0.0f) {
                    return fail(FISSION_SKIA_STATUS_INVALID_ARGUMENT, "execute_frame",
                                "invalid paragraph draw", error);
                }
                const auto status = fission_skia_paragraph_validate_draw(
                    fission_skia_paragraph_handle_from_frame_op(op), op.rect.x, op.rect.y,
                    op.radius, error);
                if (status != FISSION_SKIA_STATUS_OK) return status;
                break;
            }
            case FISSION_SKIA_FRAME_DRAW_IMAGE: {
                const auto status = validate_image_draw(op.image, error);
                if (status != FISSION_SKIA_STATUS_OK) return status;
                break;
            }
            case FISSION_SKIA_FRAME_BACKDROP_BLUR:
                if (!valid_rect(op.rect) ||
                    !std::isfinite(op.rect.x + op.rect.width) ||
                    !std::isfinite(op.rect.y + op.rect.height) ||
                    !std::isfinite(op.radius) ||
                    op.radius < 0.0f || !std::isfinite(op.sigma) || op.sigma < 0.0f) {
                    return fail(FISSION_SKIA_STATUS_INVALID_ARGUMENT, "execute_frame",
                                "invalid backdrop blur", error);
                }
                break;
            default:
                return fail(FISSION_SKIA_STATUS_UNSUPPORTED, "execute_frame",
                            "unknown operation", error);
        }
    }
    if (save_depth)
        return fail(FISSION_SKIA_STATUS_INVALID_ARGUMENT, "execute_frame",
                    "unrestored save or opacity layer", error);
    std::lock_guard<std::mutex> lock(state().mutex);
    auto found = state().surfaces.find(id);
    if (found == state().surfaces.end())
        return fail(FISSION_SKIA_STATUS_INVALID_HANDLE, "execute_frame", "invalid surface", error);
    auto status = owner(found->second, "execute_frame", error);
    if (status) return status;
    std::vector<SavedKind> saved;
    std::vector<OpacityLayer> layers;
    for (size_t index = 0; index < frame->operation_count; ++index) {
        const auto& op = frame->operations[index];
        if (op.kind == FISSION_SKIA_FRAME_CLEAR) {
            auto target = current_target(found->second, layers);
            paint_rect(target, {0, 0, static_cast<float>(found->second.width),
                                static_cast<float>(found->second.height)}, op.paint.color, true);
        } else if (op.kind == FISSION_SKIA_FRAME_SAVE) {
            saved.push_back(SavedKind::kCanvasState);
        } else if (op.kind == FISSION_SKIA_FRAME_OPACITY_LAYER) {
            layers.push_back(OpacityLayer{
                op.rect,
                op.opacity,
                std::vector<uint8_t>(found->second.pixels.size(), 0),
            });
            saved.push_back(SavedKind::kOpacityLayer);
        } else if (op.kind == FISSION_SKIA_FRAME_RESTORE) {
            const SavedKind kind = saved.back();
            saved.pop_back();
            if (kind == SavedKind::kOpacityLayer) {
                OpacityLayer layer = std::move(layers.back());
                layers.pop_back();
                auto target = current_target(found->second, layers);
                composite_layer(target, layer);
            }
        } else if (op.kind == FISSION_SKIA_FRAME_FILL_RECT) {
            auto target = current_target(found->second, layers);
            paint_rect(target, op.rect, representative_color(*frame, op.paint));
        } else if (op.kind == FISSION_SKIA_FRAME_DRAW_PARAGRAPH) {
            auto target = current_target(found->second, layers);
            status = fission_skia_paragraph_draw_test_picture(
                fission_skia_paragraph_handle_from_frame_op(op), op.rect.x, op.rect.y,
                op.radius, &target, paint_paragraph_rect, error);
            if (status != FISSION_SKIA_STATUS_OK) return status;
        } else if (op.kind == FISSION_SKIA_FRAME_DRAW_IMAGE) {
            const auto image = state().images.find(op.image.image);
            if (image == state().images.end()) {
                return fail(FISSION_SKIA_STATUS_INVALID_HANDLE, "execute_frame",
                            "image was destroyed before playback", error);
            }
            auto target = current_target(found->second, layers);
            draw_image(target, image->second, op.image);
        } else if (op.kind == FISSION_SKIA_FRAME_BACKDROP_BLUR) {
            auto target = current_target(found->second, layers);
            blur_backdrop(target, op.rect, op.radius, op.sigma);
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

fission_skia_status_t fission_skia_image_decode_encoded(
    const uint8_t* encoded, size_t encoded_length, size_t max_decoded_bytes,
    fission_skia_image_handle_t* output, fission_skia_image_info_t* info,
    fission_skia_error_t* error) {
    if (!encoded || encoded_length < 12 || !max_decoded_bytes || !output || !info ||
        info->struct_size != sizeof(*info)) {
        return fail(FISSION_SKIA_STATUS_INVALID_ARGUMENT, "image_decode_encoded",
                    "invalid encoded image or outputs", error);
    }
    *output = 0;
    info->width = 0;
    info->height = 0;
    info->reserved = 0;
    info->approximate_decoded_bytes = 0;
    // The deterministic test-only format is: "FSIM", little-endian width and
    // height, then tightly packed unpremultiplied RGBA8 pixels.
    if (std::memcmp(encoded, "FSIM", 4) != 0) {
        return fail(FISSION_SKIA_STATUS_UNSUPPORTED, "image_decode_encoded",
                    "test shim only accepts the deterministic FSIM image format", error);
    }
    const uint32_t width = read_u32_le(encoded + 4);
    const uint32_t height = read_u32_le(encoded + 8);
    if (!width || !height || static_cast<size_t>(width) > static_cast<size_t>(-1) / height ||
        static_cast<size_t>(width) * height > static_cast<size_t>(-1) / 4) {
        return fail(FISSION_SKIA_STATUS_INVALID_ARGUMENT, "image_decode_encoded",
                    "test image dimensions overflow", error);
    }
    const size_t pixel_bytes = static_cast<size_t>(width) * height * 4;
    if (pixel_bytes > static_cast<size_t>(-1) - 12) {
        return fail(FISSION_SKIA_STATUS_INVALID_ARGUMENT, "image_decode_encoded",
                    "test image encoded length overflows", error);
    }
    if (pixel_bytes > max_decoded_bytes) {
        return fail(FISSION_SKIA_STATUS_OUT_OF_MEMORY, "image_decode_encoded",
                    "decoded image exceeds the caller byte limit", error);
    }
    if (encoded_length != 12 + pixel_bytes) {
        return fail(FISSION_SKIA_STATUS_INVALID_ARGUMENT, "image_decode_encoded",
                    "test image pixel length does not match its dimensions", error);
    }
    DecodedImage image;
    image.width = width;
    image.height = height;
    image.pixels.assign(encoded + 12, encoded + encoded_length);
    const auto id = handle();
    std::lock_guard<std::mutex> lock(state().mutex);
    auto [found, inserted] = state().images.emplace(id, std::move(image));
    if (!inserted) {
        return fail(FISSION_SKIA_STATUS_INTERNAL, "image_decode_encoded",
                    "test image handle collision", error);
    }
    write_image_info(found->second, info);
    *output = id;
    clear(error);
    return FISSION_SKIA_STATUS_OK;
}

fission_skia_status_t fission_skia_image_get_info(
    fission_skia_image_handle_t id, fission_skia_image_info_t* info,
    fission_skia_error_t* error) {
    if (!info || info->struct_size != sizeof(*info)) {
        return fail(FISSION_SKIA_STATUS_INVALID_ARGUMENT, "image_get_info",
                    "invalid image info output", error);
    }
    std::lock_guard<std::mutex> lock(state().mutex);
    const auto found = state().images.find(id);
    if (found == state().images.end()) {
        return fail(FISSION_SKIA_STATUS_INVALID_HANDLE, "image_get_info",
                    "invalid image handle", error);
    }
    write_image_info(found->second, info);
    clear(error);
    return FISSION_SKIA_STATUS_OK;
}

fission_skia_status_t fission_skia_image_destroy(
    fission_skia_image_handle_t id, fission_skia_error_t* error) {
    std::lock_guard<std::mutex> lock(state().mutex);
    const auto found = state().images.find(id);
    if (found == state().images.end()) {
        return fail(FISSION_SKIA_STATUS_INVALID_HANDLE, "image_destroy",
                    "invalid image handle", error);
    }
    state().images.erase(found);
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
    counts->images = state().images.size();
    clear(error);
    return FISSION_SKIA_STATUS_OK;
}

}  // extern "C"
