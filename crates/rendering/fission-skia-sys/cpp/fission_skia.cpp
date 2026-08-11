#include "fission_skia.h"
#include "fission_skia_paragraph_internal.h"
#ifndef FISSION_SKIA_ENABLE_GANESH_VULKAN
#define FISSION_SKIA_ENABLE_GANESH_VULKAN 0
#endif
#if FISSION_SKIA_ENABLE_GANESH_VULKAN
#include "fission_skia_ganesh_vulkan.h"
#endif

#include "include/core/SkCanvas.h"
#include "include/core/SkColorSpace.h"
#include "include/core/SkData.h"
#include "include/core/SkGraphics.h"
#include "include/core/SkImage.h"
#include "include/core/SkImageInfo.h"
#include "include/core/SkMaskFilter.h"
#include "include/core/SkMatrix.h"
#include "include/core/SkPaint.h"
#include "include/core/SkPath.h"
#include "include/core/SkPathBuilder.h"
#include "include/core/SkPicture.h"
#include "include/core/SkPictureRecorder.h"
#include "include/core/SkRRect.h"
#include "include/core/SkSamplingOptions.h"
#include "include/core/SkSize.h"
#include "include/core/SkStream.h"
#include "include/core/SkSurface.h"
#include "include/codec/SkCodec.h"
#include "include/codec/SkEncodedOrigin.h"
#include "include/effects/SkDashPathEffect.h"
#include "include/effects/SkGradient.h"
#include "include/effects/SkImageFilters.h"
#include "modules/svg/include/SkSVGDOM.h"

#include <algorithm>
#include <atomic>
#include <cmath>
#include <cstring>
#include <limits>
#include <memory>
#include <mutex>
#include <new>
#include <thread>
#include <tuple>
#include <unordered_map>
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

enum class ContextBackend { kRaster, kGaneshVulkan };
enum class SurfaceBackend { kRaster, kGaneshVulkan };

struct EngineState {
    std::thread::id owner;
    uint64_t live_contexts = 0;
};

struct ContextState {
    std::thread::id owner;
    fission_skia_engine_handle_t engine = 0;
    uint64_t live_surfaces = 0;
    ContextBackend backend = ContextBackend::kRaster;
#if FISSION_SKIA_ENABLE_GANESH_VULKAN
    std::unique_ptr<fission::skia::ganesh::VulkanContext> ganesh;
#endif
};

struct SurfaceState {
    std::thread::id owner;
    fission_skia_context_handle_t context = 0;
    uint32_t width = 0;
    uint32_t height = 0;
    SurfaceBackend backend = SurfaceBackend::kRaster;
    sk_sp<SkSurface> surface;
#if FISSION_SKIA_ENABLE_GANESH_VULKAN
    std::unique_ptr<fission::skia::ganesh::VulkanSurface> ganesh;
#endif
};

struct ImageState {
    uint32_t width = 0;
    uint32_t height = 0;
    size_t approximate_decoded_bytes = 0;
    sk_sp<SkImage> image;
};

struct SvgDocumentState {
    SkSize intrinsic_size = SkSize::Make(0.0f, 0.0f);
    sk_sp<SkSVGDOM> document;
};

struct PictureState {
    sk_sp<SkPicture> picture;
};

struct Registry {
    std::mutex mutex;
    std::unordered_map<uint64_t, std::unique_ptr<EngineState>> engines;
    std::unordered_map<uint64_t, std::unique_ptr<ContextState>> contexts;
    std::unordered_map<uint64_t, std::unique_ptr<SurfaceState>> surfaces;
    std::unordered_map<uint64_t, std::unique_ptr<ImageState>> images;
    std::unordered_map<uint64_t, std::unique_ptr<SvgDocumentState>> svg_documents;
    std::unordered_map<uint64_t, std::unique_ptr<PictureState>> pictures;
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

uint8_t ascii_lower(uint8_t value) {
    return value >= 'A' && value <= 'Z'
        ? static_cast<uint8_t>(value + ('a' - 'A'))
        : value;
}

bool contains_ascii_case_insensitive(
    const uint8_t* bytes,
    size_t length,
    const char* needle) {
    const size_t needle_length = std::strlen(needle);
    if (needle_length == 0 || needle_length > length) return false;
    for (size_t offset = 0; offset <= length - needle_length; ++offset) {
        bool matches = true;
        for (size_t index = 0; index < needle_length; ++index) {
            if (ascii_lower(bytes[offset + index]) !=
                ascii_lower(static_cast<uint8_t>(needle[index]))) {
                matches = false;
                break;
            }
        }
        if (matches) return true;
    }
    return false;
}

bool valid_utf8_without_nul(const uint8_t* bytes, size_t length) {
    size_t index = 0;
    while (index < length) {
        const uint8_t first = bytes[index];
        if (first == 0) return false;
        if (first <= 0x7f) {
            index += 1;
            continue;
        }
        if (first >= 0xc2 && first <= 0xdf) {
            if (index + 1 >= length || (bytes[index + 1] & 0xc0) != 0x80) return false;
            index += 2;
            continue;
        }
        if (first >= 0xe0 && first <= 0xef) {
            if (index + 2 >= length || (bytes[index + 2] & 0xc0) != 0x80) return false;
            const uint8_t second = bytes[index + 1];
            if ((first == 0xe0 && (second < 0xa0 || second > 0xbf)) ||
                (first == 0xed && (second < 0x80 || second > 0x9f)) ||
                (first != 0xe0 && first != 0xed && (second & 0xc0) != 0x80)) {
                return false;
            }
            index += 3;
            continue;
        }
        if (first >= 0xf0 && first <= 0xf4) {
            if (index + 3 >= length || (bytes[index + 2] & 0xc0) != 0x80 ||
                (bytes[index + 3] & 0xc0) != 0x80) {
                return false;
            }
            const uint8_t second = bytes[index + 1];
            if ((first == 0xf0 && (second < 0x90 || second > 0xbf)) ||
                (first == 0xf4 && (second < 0x80 || second > 0x8f)) ||
                (first != 0xf0 && first != 0xf4 && (second & 0xc0) != 0x80)) {
                return false;
            }
            index += 4;
            continue;
        }
        return false;
    }
    return true;
}

bool valid_svg_source(const uint8_t* bytes, size_t length) {
    return bytes != nullptr && length != 0 &&
           length <= FISSION_SKIA_MAX_SVG_DOCUMENT_BYTES &&
           valid_utf8_without_nul(bytes, length) &&
           !contains_ascii_case_insensitive(bytes, length, "<!doctype") &&
           !contains_ascii_case_insensitive(bytes, length, "<!entity");
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

bool valid_non_empty_rect(const fission_skia_rect_t& rect) {
    return valid_rect(rect) && rect.width > 0.0f && rect.height > 0.0f;
}

bool valid_native_window(const fission_skia_native_window_t* window) {
    if (window == nullptr || window->struct_size != sizeof(*window) ||
        window->display == 0 || window->window == 0 ||
        window->display > static_cast<uint64_t>(UINTPTR_MAX)) {
        return false;
    }
    switch (window->kind) {
        case FISSION_SKIA_NATIVE_WINDOW_WAYLAND:
            return window->window <= static_cast<uint64_t>(UINTPTR_MAX) &&
                   window->visual_id == 0;
        case FISSION_SKIA_NATIVE_WINDOW_XLIB:
            return window->window <= static_cast<uint64_t>(UINTPTR_MAX) &&
                   window->visual_id <= static_cast<uint64_t>(UINTPTR_MAX);
        case FISSION_SKIA_NATIVE_WINDOW_XCB:
            return window->window <= UINT32_MAX && window->visual_id <= UINT32_MAX;
        default:
            return false;
    }
}

void write_image_info(
    const ImageState& image,
    fission_skia_image_info_t* out_info) {
    out_info->width = image.width;
    out_info->height = image.height;
    out_info->reserved = 0;
    out_info->approximate_decoded_bytes = image.approximate_decoded_bytes;
}

bool valid_point(const fission_skia_point_t& point) {
    return finite(point.x) && finite(point.y);
}

bool valid_affine(const fission_skia_affine_t& affine) {
    const float values[] = {
        affine.scale_x, affine.skew_x, affine.translate_x,
        affine.skew_y, affine.scale_y, affine.translate_y,
    };
    for (float value : values) {
        if (!finite(value)) return false;
    }
    return true;
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

bool valid_range(uint32_t offset, uint32_t count, size_t length) {
    const size_t start = offset;
    const size_t amount = count;
    return start <= length && amount <= length - start;
}

fission_skia_status_t validate_paint(
    const fission_skia_frame_t& frame,
    const fission_skia_paint_t& paint,
    fission_skia_error_t* error) {
    if (paint.struct_size != sizeof(fission_skia_paint_t)) {
        return fail(FISSION_SKIA_STATUS_INVALID_ARGUMENT, "execute_frame",
                    "paint has an incompatible layout", error);
    }
    if (paint.kind == FISSION_SKIA_PAINT_SOLID) {
        return valid_color(paint.color)
            ? FISSION_SKIA_STATUS_OK
            : fail(FISSION_SKIA_STATUS_INVALID_ARGUMENT, "execute_frame",
                   "solid paint has invalid sRGB components", error);
    }
    if (paint.kind != FISSION_SKIA_PAINT_LINEAR_GRADIENT &&
        paint.kind != FISSION_SKIA_PAINT_RADIAL_GRADIENT) {
        return fail(FISSION_SKIA_STATUS_INVALID_ARGUMENT, "execute_frame",
                    "paint has an unknown kind", error);
    }
    if (!valid_point(paint.start) ||
        (paint.kind == FISSION_SKIA_PAINT_LINEAR_GRADIENT && !valid_point(paint.end)) ||
        (paint.kind == FISSION_SKIA_PAINT_RADIAL_GRADIENT &&
         (!finite(paint.radius) || paint.radius < 0.0f))) {
        return fail(FISSION_SKIA_STATUS_INVALID_ARGUMENT, "execute_frame",
                    "gradient geometry is invalid", error);
    }
    if (!valid_range(paint.stop_offset, paint.stop_count, frame.gradient_stop_count)) {
        return fail(FISSION_SKIA_STATUS_INVALID_ARGUMENT, "execute_frame",
                    "gradient stop range is outside the frame", error);
    }
    float previous = 0.0f;
    for (uint32_t index = 0; index < paint.stop_count; ++index) {
        const auto& stop = frame.gradient_stops[paint.stop_offset + index];
        if (!finite(stop.offset) || stop.offset < 0.0f || stop.offset > 1.0f ||
            !valid_color(stop.color) || (index != 0 && stop.offset < previous)) {
            return fail(FISSION_SKIA_STATUS_INVALID_ARGUMENT, "execute_frame",
                        "gradient stops must be ordered finite sRGB stops in 0..=1", error);
        }
        previous = stop.offset;
    }
    return FISSION_SKIA_STATUS_OK;
}

fission_skia_status_t validate_stroke(
    const fission_skia_frame_t& frame,
    const fission_skia_stroke_t& stroke,
    fission_skia_error_t* error) {
    if (stroke.struct_size != sizeof(fission_skia_stroke_t) ||
        !finite(stroke.width) || stroke.width < 0.0f ||
        stroke.line_cap < FISSION_SKIA_LINE_CAP_BUTT ||
        stroke.line_cap > FISSION_SKIA_LINE_CAP_SQUARE ||
        stroke.line_join < FISSION_SKIA_LINE_JOIN_MITER ||
        stroke.line_join > FISSION_SKIA_LINE_JOIN_BEVEL ||
        !valid_range(stroke.dash_offset, stroke.dash_count, frame.dash_interval_count) ||
        stroke.dash_count % 2 != 0) {
        return fail(FISSION_SKIA_STATUS_INVALID_ARGUMENT, "execute_frame",
                    "stroke layout, geometry, or dash range is invalid", error);
    }
    float dash_sum = 0.0f;
    for (uint32_t index = 0; index < stroke.dash_count; ++index) {
        const float interval = frame.dash_intervals[stroke.dash_offset + index];
        if (!finite(interval) || interval < 0.0f) {
            return fail(FISSION_SKIA_STATUS_INVALID_ARGUMENT, "execute_frame",
                        "stroke dash intervals must be finite and non-negative", error);
        }
        dash_sum += interval;
    }
    if (stroke.dash_count != 0 && (!finite(dash_sum) || dash_sum <= 0.0f)) {
        return fail(FISSION_SKIA_STATUS_INVALID_ARGUMENT, "execute_frame",
                    "stroke dash intervals must have a positive finite sum", error);
    }
    return FISSION_SKIA_STATUS_OK;
}

fission_skia_status_t validate_shadow(
    const fission_skia_box_shadow_t& shadow,
    fission_skia_error_t* error) {
    if (shadow.struct_size != sizeof(fission_skia_box_shadow_t) ||
        shadow.inset > 1 || !valid_color(shadow.color) ||
        !finite(shadow.blur_radius) || shadow.blur_radius < 0.0f ||
        !finite(shadow.spread_radius) || !finite(shadow.offset_x) ||
        !finite(shadow.offset_y)) {
        return fail(FISSION_SKIA_STATUS_INVALID_ARGUMENT, "execute_frame",
                    "box shadow contains invalid geometry or color", error);
    }
    return FISSION_SKIA_STATUS_OK;
}

fission_skia_status_t validate_image_draw(
    const fission_skia_image_draw_t& draw,
    fission_skia_error_t* error) {
    if (draw.struct_size != sizeof(fission_skia_image_draw_t) ||
        (draw.sampling != FISSION_SKIA_IMAGE_SAMPLING_NEAREST &&
         draw.sampling != FISSION_SKIA_IMAGE_SAMPLING_LINEAR) ||
        !valid_non_empty_rect(draw.source) ||
        !valid_non_empty_rect(draw.destination) ||
        draw.source.x < 0.0f || draw.source.y < 0.0f) {
        return fail(FISSION_SKIA_STATUS_INVALID_ARGUMENT, "execute_frame",
                    "image draw has an invalid layout, rectangle, or sampling mode", error);
    }
    std::lock_guard<std::mutex> lock(registry().mutex);
    const auto found = registry().images.find(draw.image);
    if (found == registry().images.end()) {
        return fail(FISSION_SKIA_STATUS_INVALID_HANDLE, "execute_frame",
                    "image draw handle is not live", error);
    }
    const double right = static_cast<double>(draw.source.x) + draw.source.width;
    const double bottom = static_cast<double>(draw.source.y) + draw.source.height;
    if (right > found->second->width || bottom > found->second->height) {
        return fail(FISSION_SKIA_STATUS_INVALID_ARGUMENT, "execute_frame",
                    "image source rectangle lies outside the decoded image", error);
    }
    return FISSION_SKIA_STATUS_OK;
}

fission_skia_status_t validate_backdrop_blur(
    const fission_skia_frame_op_t& operation,
    fission_skia_error_t* error) {
    if (!valid_rect(operation.rect) ||
        !finite(operation.rect.x + operation.rect.width) ||
        !finite(operation.rect.y + operation.rect.height) ||
        !finite(operation.radius) ||
        operation.radius < 0.0f || !finite(operation.sigma) ||
        operation.sigma < 0.0f) {
        return fail(FISSION_SKIA_STATUS_INVALID_ARGUMENT, "execute_frame",
                    "backdrop blur has invalid bounds, radius, or sigma", error);
    }
    return FISSION_SKIA_STATUS_OK;
}

fission_skia_status_t validate_svg_draw(
    const fission_skia_svg_draw_t& draw,
    fission_skia_error_t* error) {
    if (draw.struct_size != sizeof(fission_skia_svg_draw_t) || draw.reserved != 0 ||
        !valid_non_empty_rect(draw.destination) ||
        !finite(draw.destination.x + draw.destination.width) ||
        !finite(draw.destination.y + draw.destination.height)) {
        return fail(FISSION_SKIA_STATUS_INVALID_ARGUMENT, "execute_frame",
                    "SVG draw has an invalid layout or destination", error);
    }
    std::lock_guard<std::mutex> lock(registry().mutex);
    if (registry().svg_documents.find(draw.document) == registry().svg_documents.end()) {
        return fail(FISSION_SKIA_STATUS_INVALID_HANDLE, "execute_frame",
                    "SVG document handle is not live", error);
    }
    return FISSION_SKIA_STATUS_OK;
}

fission_skia_status_t validate_picture_draw(
    const fission_skia_picture_draw_t& draw,
    fission_skia_error_t* error) {
    if (draw.struct_size != sizeof(fission_skia_picture_draw_t) || draw.reserved != 0) {
        return fail(FISSION_SKIA_STATUS_INVALID_ARGUMENT, "execute_frame",
                    "picture draw has an invalid layout", error);
    }
    std::lock_guard<std::mutex> lock(registry().mutex);
    if (registry().pictures.find(draw.picture) == registry().pictures.end()) {
        return fail(FISSION_SKIA_STATUS_INVALID_HANDLE, "execute_frame",
                    "picture draw handle is not live", error);
    }
    return FISSION_SKIA_STATUS_OK;
}

fission_skia_status_t validate_path(
    const fission_skia_frame_t& frame,
    const fission_skia_frame_op_t& operation,
    fission_skia_error_t* error) {
    if (operation.fill_rule != FISSION_SKIA_FILL_NON_ZERO &&
        operation.fill_rule != FISSION_SKIA_FILL_EVEN_ODD) {
        return fail(FISSION_SKIA_STATUS_INVALID_ARGUMENT, "execute_frame",
                    "path has an unknown fill rule", error);
    }
    const size_t offset = operation.path_offset;
    const size_t count = operation.path_count;
    if (count == 0 || offset > frame.path_command_count ||
        count > frame.path_command_count - offset) {
        return fail(FISSION_SKIA_STATUS_INVALID_ARGUMENT, "execute_frame",
                    "path command range is outside the frame", error);
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
    bool recording,
    fission_skia_error_t* error) {
    if (frame == nullptr || frame->struct_size != sizeof(fission_skia_frame_t)) {
        return fail(FISSION_SKIA_STATUS_INVALID_ARGUMENT, "execute_frame",
                    "frame is null or has an incompatible layout", error);
    }
    if ((frame->operation_count != 0 && frame->operations == nullptr) ||
        (frame->path_command_count != 0 && frame->path_commands == nullptr) ||
        (frame->gradient_stop_count != 0 && frame->gradient_stops == nullptr) ||
        (frame->dash_interval_count != 0 && frame->dash_intervals == nullptr)) {
        return fail(FISSION_SKIA_STATUS_INVALID_ARGUMENT, "execute_frame",
                    "a non-empty frame array has a null pointer", error);
    }
    size_t save_depth = 0;
    for (size_t index = 0; index < frame->operation_count; ++index) {
        const auto& operation = frame->operations[index];
        if (operation.struct_size != sizeof(fission_skia_frame_op_t)) {
            return fail(FISSION_SKIA_STATUS_INVALID_ARGUMENT, "execute_frame",
                        "frame operation has an incompatible layout", error);
        }
        switch (operation.kind) {
            case FISSION_SKIA_FRAME_CLEAR: {
                if (recording) {
                    return fail(FISSION_SKIA_STATUS_UNSUPPORTED, "execute_frame",
                                "clear cannot be recorded because it targets the destination surface",
                                error);
                }
                const auto status = validate_paint(*frame, operation.paint, error);
                if (status != FISSION_SKIA_STATUS_OK ||
                    operation.paint.kind != FISSION_SKIA_PAINT_SOLID) {
                    return status != FISSION_SKIA_STATUS_OK
                        ? status
                        : fail(FISSION_SKIA_STATUS_INVALID_ARGUMENT, "execute_frame",
                               "clear requires a solid color", error);
                }
                break;
            }
            case FISSION_SKIA_FRAME_SAVE:
                save_depth += 1;
                break;
            case FISSION_SKIA_FRAME_OPACITY_LAYER:
                if (!valid_rect(operation.rect) || !finite(operation.opacity) ||
                    operation.opacity < 0.0f || operation.opacity > 1.0f) {
                    return fail(FISSION_SKIA_STATUS_INVALID_ARGUMENT, "execute_frame",
                                "opacity layer has invalid bounds or alpha", error);
                }
                save_depth += 1;
                break;
            case FISSION_SKIA_FRAME_RESTORE:
                if (save_depth == 0) {
                    return fail(FISSION_SKIA_STATUS_INVALID_ARGUMENT, "execute_frame",
                                "restore has no matching save or opacity layer", error);
                }
                save_depth -= 1;
                break;
            case FISSION_SKIA_FRAME_CLIP_RECT:
            case FISSION_SKIA_FRAME_CLIP_ROUNDED_RECT:
                if (!valid_rect(operation.rect) || !finite(operation.radius) ||
                    operation.radius < 0.0f) {
                    return fail(FISSION_SKIA_STATUS_INVALID_ARGUMENT, "execute_frame",
                                "clip rectangle has invalid geometry", error);
                }
                break;
            case FISSION_SKIA_FRAME_CONCAT_AFFINE:
                if (!valid_affine(operation.affine)) {
                    return fail(FISSION_SKIA_STATUS_INVALID_ARGUMENT, "execute_frame",
                                "affine transform contains a non-finite value", error);
                }
                break;
            case FISSION_SKIA_FRAME_FILL_RECT: {
                if (!valid_rect(operation.rect) || !finite(operation.radius) ||
                    operation.radius < 0.0f) {
                    return fail(FISSION_SKIA_STATUS_INVALID_ARGUMENT, "execute_frame",
                                "fill rectangle has invalid geometry", error);
                }
                const auto status = validate_paint(*frame, operation.paint, error);
                if (status != FISSION_SKIA_STATUS_OK) return status;
                break;
            }
            case FISSION_SKIA_FRAME_STROKE_RECT: {
                if (!valid_rect(operation.rect) || !finite(operation.radius) ||
                    operation.radius < 0.0f) {
                    return fail(FISSION_SKIA_STATUS_INVALID_ARGUMENT, "execute_frame",
                                "stroke rectangle has invalid geometry", error);
                }
                auto status = validate_paint(*frame, operation.paint, error);
                if (status != FISSION_SKIA_STATUS_OK) return status;
                status = validate_stroke(*frame, operation.stroke, error);
                if (status != FISSION_SKIA_STATUS_OK) return status;
                break;
            }
            case FISSION_SKIA_FRAME_FILL_PATH:
            case FISSION_SKIA_FRAME_STROKE_PATH: {
                const auto status = validate_path(*frame, operation, error);
                if (status != FISSION_SKIA_STATUS_OK) {
                    return status;
                }
                auto paint_status = validate_paint(*frame, operation.paint, error);
                if (paint_status != FISSION_SKIA_STATUS_OK) return paint_status;
                if (operation.kind == FISSION_SKIA_FRAME_STROKE_PATH) {
                    paint_status = validate_stroke(*frame, operation.stroke, error);
                    if (paint_status != FISSION_SKIA_STATUS_OK) return paint_status;
                }
                break;
            }
            case FISSION_SKIA_FRAME_BOX_SHADOW: {
                if (!valid_rect(operation.rect) || !finite(operation.radius) ||
                    operation.radius < 0.0f) {
                    return fail(FISSION_SKIA_STATUS_INVALID_ARGUMENT, "execute_frame",
                                "shadow rectangle has invalid geometry", error);
                }
                const auto status = validate_shadow(operation.shadow, error);
                if (status != FISSION_SKIA_STATUS_OK) return status;
                break;
            }
            case FISSION_SKIA_FRAME_DRAW_PARAGRAPH: {
                if (!finite(operation.rect.x) || !finite(operation.rect.y) ||
                    operation.rect.width != 0.0f || operation.rect.height != 0.0f ||
                    !finite(operation.radius) || operation.radius <= 0.0f) {
                    return fail(FISSION_SKIA_STATUS_INVALID_ARGUMENT, "execute_frame",
                                "paragraph draw has an invalid origin or scale factor", error);
                }
                const auto status = fission_skia_paragraph_validate_draw(
                    fission_skia_paragraph_handle_from_frame_op(operation), operation.rect.x,
                    operation.rect.y, operation.radius, error);
                if (status != FISSION_SKIA_STATUS_OK) return status;
                break;
            }
            case FISSION_SKIA_FRAME_DRAW_IMAGE: {
                const auto status = validate_image_draw(operation.image, error);
                if (status != FISSION_SKIA_STATUS_OK) return status;
                break;
            }
            case FISSION_SKIA_FRAME_BACKDROP_BLUR: {
                if (recording) {
                    return fail(FISSION_SKIA_STATUS_UNSUPPORTED, "execute_frame",
                                "backdrop blur cannot be recorded because it reads destination pixels",
                                error);
                }
                const auto status = validate_backdrop_blur(operation, error);
                if (status != FISSION_SKIA_STATUS_OK) return status;
                break;
            }
            case FISSION_SKIA_FRAME_DRAW_SVG: {
                const auto status = validate_svg_draw(operation.svg, error);
                if (status != FISSION_SKIA_STATUS_OK) return status;
                break;
            }
            case FISSION_SKIA_FRAME_DRAW_PICTURE: {
                const auto status = validate_picture_draw(operation.picture, error);
                if (status != FISSION_SKIA_STATUS_OK) return status;
                break;
            }
            default:
                return fail(FISSION_SKIA_STATUS_UNSUPPORTED, "execute_frame",
                            "frame operation is not supported by this ABI", error);
        }
    }
    if (save_depth != 0) {
        return fail(FISSION_SKIA_STATUS_INVALID_ARGUMENT, "execute_frame",
                    "frame leaves save or opacity-layer operations unrestored", error);
    }
    return FISSION_SKIA_STATUS_OK;
}

SkColor4f sk_color(const fission_skia_color_t& color) {
    return {color.red, color.green, color.blue, color.alpha};
}

SkRect sk_rect(const fission_skia_rect_t& rect) {
    return SkRect::MakeXYWH(rect.x, rect.y, rect.width, rect.height);
}

SkRRect sk_rounded_rect(const fission_skia_rect_t& rect, float radius) {
    return SkRRect::MakeRectXY(sk_rect(rect), radius, radius);
}

bool make_positions_strict(std::vector<SkScalar>* positions) {
    if (positions->size() < 2) return true;
    for (size_t index = 1; index < positions->size(); ++index) {
        if ((*positions)[index] <= (*positions)[index - 1]) {
            (*positions)[index] = std::nextafter((*positions)[index - 1],
                                                 std::numeric_limits<float>::infinity());
        }
    }
    if (positions->back() > 1.0f) {
        positions->back() = 1.0f;
        for (size_t index = positions->size() - 1; index-- > 0;) {
            if ((*positions)[index] >= (*positions)[index + 1]) {
                (*positions)[index] = std::nextafter((*positions)[index + 1],
                                                     -std::numeric_limits<float>::infinity());
            }
        }
    }
    return positions->front() >= 0.0f;
}

bool configure_paint(
    const fission_skia_frame_t& frame,
    const fission_skia_paint_t& source,
    SkPaint* paint) {
    paint->setAntiAlias(true);
    auto srgb = SkColorSpace::MakeSRGB();
    if (source.kind == FISSION_SKIA_PAINT_SOLID) {
        paint->setColor4f(sk_color(source.color), srgb.get());
        return true;
    }

    if (source.stop_count == 0) {
        paint->setColor4f({0.0f, 0.0f, 0.0f, 0.0f}, srgb.get());
        return true;
    }
    const auto* stops = frame.gradient_stops + source.stop_offset;
    if (source.stop_count == 1) {
        paint->setColor4f(sk_color(stops[0].color), srgb.get());
        return true;
    }

    std::vector<SkColor4f> colors;
    std::vector<SkScalar> positions;
    colors.reserve(source.stop_count);
    positions.reserve(source.stop_count);
    for (uint32_t index = 0; index < source.stop_count; ++index) {
        colors.push_back(sk_color(stops[index].color));
        positions.push_back(stops[index].offset);
    }
    if (!make_positions_strict(&positions)) return false;

    sk_sp<SkShader> shader;
    if (source.kind == FISSION_SKIA_PAINT_LINEAR_GRADIENT) {
        if (source.start.x == source.end.x && source.start.y == source.end.y) {
            paint->setColor4f(colors.back(), srgb.get());
            return true;
        }
        const SkPoint points[] = {
            {source.start.x, source.start.y},
            {source.end.x, source.end.y},
        };
        const SkGradient::Colors gradient_colors(
            SkSpan<const SkColor4f>(colors.data(), colors.size()),
            SkSpan<const float>(positions.data(), positions.size()),
            SkTileMode::kClamp, srgb);
        const SkGradient gradient(gradient_colors, SkGradient::Interpolation{});
        shader = SkShaders::LinearGradient(points, gradient);
    } else {
        if (source.radius <= 0.0f) {
            paint->setColor4f(colors.back(), srgb.get());
            return true;
        }
        const SkGradient::Colors gradient_colors(
            SkSpan<const SkColor4f>(colors.data(), colors.size()),
            SkSpan<const float>(positions.data(), positions.size()),
            SkTileMode::kClamp, srgb);
        const SkGradient gradient(gradient_colors, SkGradient::Interpolation{});
        shader = SkShaders::RadialGradient(
            {source.start.x, source.start.y}, source.radius, gradient);
    }
    if (!shader) return false;
    paint->setShader(std::move(shader));
    return true;
}

bool configure_stroke(
    const fission_skia_frame_t& frame,
    const fission_skia_stroke_t& source,
    SkPaint* paint) {
    paint->setStyle(SkPaint::kStroke_Style);
    paint->setStrokeWidth(source.width);
    switch (source.line_cap) {
        case FISSION_SKIA_LINE_CAP_BUTT: paint->setStrokeCap(SkPaint::kButt_Cap); break;
        case FISSION_SKIA_LINE_CAP_ROUND: paint->setStrokeCap(SkPaint::kRound_Cap); break;
        case FISSION_SKIA_LINE_CAP_SQUARE: paint->setStrokeCap(SkPaint::kSquare_Cap); break;
    }
    switch (source.line_join) {
        case FISSION_SKIA_LINE_JOIN_MITER: paint->setStrokeJoin(SkPaint::kMiter_Join); break;
        case FISSION_SKIA_LINE_JOIN_ROUND: paint->setStrokeJoin(SkPaint::kRound_Join); break;
        case FISSION_SKIA_LINE_JOIN_BEVEL: paint->setStrokeJoin(SkPaint::kBevel_Join); break;
    }
    if (source.dash_count != 0) {
        auto effect = SkDashPathEffect::Make(
            SkSpan<const SkScalar>(frame.dash_intervals + source.dash_offset,
                                   source.dash_count),
            0.0f);
        if (!effect) return false;
        paint->setPathEffect(std::move(effect));
    }
    return true;
}

bool configure_shadow_paint(
    const fission_skia_box_shadow_t& shadow,
    SkPaint* paint) {
    paint->setAntiAlias(true);
    auto srgb = SkColorSpace::MakeSRGB();
    paint->setColor4f(sk_color(shadow.color), srgb.get());
    const float sigma = shadow.blur_radius * 0.5f;
    if (sigma > 0.0f) {
        auto filter = SkMaskFilter::MakeBlur(kNormal_SkBlurStyle, sigma, true);
        if (!filter) return false;
        paint->setMaskFilter(std::move(filter));
    }
    return true;
}

bool draw_box_shadow(
    SkCanvas* canvas,
    const fission_skia_rect_t& rect,
    float radius,
    const fission_skia_box_shadow_t& shadow) {
    SkPaint paint;
    if (!configure_shadow_paint(shadow, &paint)) return false;

    if (shadow.inset == 0) {
        fission_skia_rect_t expanded = {
            rect.x + shadow.offset_x - shadow.spread_radius,
            rect.y + shadow.offset_y - shadow.spread_radius,
            std::max(0.0f, rect.width + shadow.spread_radius * 2.0f),
            std::max(0.0f, rect.height + shadow.spread_radius * 2.0f),
        };
        const float expanded_radius = std::max(0.0f, radius + shadow.spread_radius);
        if (!valid_rect(expanded) || !finite(expanded_radius)) return false;
        canvas->drawRRect(sk_rounded_rect(expanded, expanded_radius), paint);
        return true;
    }

    fission_skia_rect_t hole = {
        rect.x + shadow.spread_radius + shadow.offset_x,
        rect.y + shadow.spread_radius + shadow.offset_y,
        std::max(0.0f, rect.width - shadow.spread_radius * 2.0f),
        std::max(0.0f, rect.height - shadow.spread_radius * 2.0f),
    };
    const float hole_radius = std::max(0.0f, radius - shadow.spread_radius);
    if (!valid_rect(hole) || !finite(hole_radius)) return false;

    canvas->save();
    canvas->clipRRect(sk_rounded_rect(rect, radius), SkClipOp::kIntersect, true);
    SkPathBuilder outside_hole;
    outside_hole.setFillType(SkPathFillType::kInverseEvenOdd);
    outside_hole.addRRect(sk_rounded_rect(hole, hole_radius));
    canvas->drawPath(outside_hole.detach(), paint);
    canvas->restore();
    return true;
}

bool draw_backdrop_blur(
    SurfaceState& surface,
    SkCanvas* canvas,
    const fission_skia_rect_t& rect,
    float radius,
    float sigma) {
    if (sigma == 0.0f || rect.width == 0.0f || rect.height == 0.0f) return true;

    SkPathBuilder local_clip;
    local_clip.addRRect(sk_rounded_rect(rect, radius));
    auto device_clip = local_clip.detach().tryMakeTransform(
        canvas->getLocalToDeviceAs3x3());
    if (!device_clip) return false;

    const SkRect device_bounds = device_clip->getBounds();
    const SkRect surface_bounds = SkRect::MakeWH(
        static_cast<SkScalar>(surface.width),
        static_cast<SkScalar>(surface.height));
    auto blur = SkImageFilters::Blur(
        sigma, sigma, SkTileMode::kClamp, nullptr, surface_bounds);
    if (!blur) return false;

    // Fission supplies device-pixel sigma and geometry. Converting the rounded
    // clip to device space before resetting the matrix keeps the shape's full
    // affine transform while preventing Skia from mapping sigma through the
    // current transform a second time.
    canvas->save();
    canvas->resetMatrix();
    canvas->clipPath(*device_clip, SkClipOp::kIntersect, true);
    canvas->saveLayer(SkCanvas::SaveLayerRec(
        &device_bounds, nullptr, blur.get(), 0));
    canvas->restore();
    canvas->restore();
    return true;
}

bool draw_svg_document(
    SkCanvas* canvas,
    SvgDocumentState& state,
    const fission_skia_rect_t& destination) {
    if (!state.document) return false;

    canvas->save();
    canvas->clipRect(sk_rect(destination), SkClipOp::kIntersect, true);
    const float intrinsic_width = state.intrinsic_size.width();
    const float intrinsic_height = state.intrinsic_size.height();
    if (intrinsic_width > 0.0f && intrinsic_height > 0.0f) {
        const float scale = std::min(
            destination.width / intrinsic_width,
            destination.height / intrinsic_height);
        const float scaled_width = intrinsic_width * scale;
        const float scaled_height = intrinsic_height * scale;
        if (!finite(scale) || scale <= 0.0f || !finite(scaled_width) ||
            !finite(scaled_height)) {
            canvas->restore();
            return false;
        }
        state.document->setContainerSize(state.intrinsic_size);
        canvas->translate(
            destination.x + (destination.width - scaled_width) * 0.5f,
            destination.y + (destination.height - scaled_height) * 0.5f);
        canvas->scale(scale, scale);
    } else {
        state.document->setContainerSize(
            SkSize::Make(destination.width, destination.height));
        canvas->translate(destination.x, destination.y);
    }
    state.document->render(canvas);
    canvas->restore();
    return true;
}

SkPath sk_path(
    const fission_skia_path_command_t* commands,
    size_t count,
    uint32_t fill_rule) {
    SkPathBuilder path;
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
    return path.detach();
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

#if FISSION_SKIA_ENABLE_GANESH_VULKAN
fission_skia_status_t cancel_ganesh_frame(
    fission::skia::ganesh::VulkanSurface& surface,
    fission_skia_status_t original_status,
    fission_skia_error_t* error) {
    const auto cancel = surface.cancel_frame();
    return cancel.ok()
        ? original_status
        : fail(cancel.status, "execute_frame", cancel.message, error);
}
#endif

fission_skia_status_t play_frame(
    SkCanvas* canvas,
    SurfaceState* surface,
    const fission_skia_frame_t& frame,
    const char* operation_name,
    fission_skia_error_t* error) {
    const int initial_save_count = canvas->getSaveCount();
    for (size_t index = 0; index < frame.operation_count; ++index) {
        const auto& operation = frame.operations[index];
        switch (operation.kind) {
            case FISSION_SKIA_FRAME_CLEAR:
                canvas->clear(sk_color(operation.paint.color));
                break;
            case FISSION_SKIA_FRAME_SAVE:
                canvas->save();
                break;
            case FISSION_SKIA_FRAME_OPACITY_LAYER: {
                SkPaint paint;
                paint.setAlphaf(operation.opacity);
                const SkRect bounds = sk_rect(operation.rect);
                canvas->saveLayer(&bounds, &paint);
                canvas->clipRect(bounds, SkClipOp::kIntersect, false);
                break;
            }
            case FISSION_SKIA_FRAME_RESTORE:
                canvas->restore();
                break;
            case FISSION_SKIA_FRAME_CLIP_RECT:
                canvas->clipRect(sk_rect(operation.rect), SkClipOp::kIntersect, true);
                break;
            case FISSION_SKIA_FRAME_CLIP_ROUNDED_RECT:
                canvas->clipRRect(sk_rounded_rect(operation.rect, operation.radius),
                                  SkClipOp::kIntersect, true);
                break;
            case FISSION_SKIA_FRAME_CONCAT_AFFINE:
                canvas->concat(SkMatrix::MakeAll(
                    operation.affine.scale_x, operation.affine.skew_x,
                    operation.affine.translate_x, operation.affine.skew_y,
                    operation.affine.scale_y, operation.affine.translate_y,
                    0.0f, 0.0f, 1.0f));
                break;
            case FISSION_SKIA_FRAME_FILL_RECT:
            case FISSION_SKIA_FRAME_STROKE_RECT: {
                if (operation.kind == FISSION_SKIA_FRAME_STROKE_RECT &&
                    operation.stroke.width == 0.0f) break;
                SkPaint paint;
                if (!configure_paint(frame, operation.paint, &paint) ||
                    (operation.kind == FISSION_SKIA_FRAME_STROKE_RECT &&
                     !configure_stroke(frame, operation.stroke, &paint))) {
                    canvas->restoreToCount(initial_save_count);
                    return fail(FISSION_SKIA_STATUS_INTERNAL, operation_name,
                                "Skia rejected validated rectangle paint", error);
                }
                paint.setStyle(operation.kind == FISSION_SKIA_FRAME_FILL_RECT
                    ? SkPaint::kFill_Style
                    : SkPaint::kStroke_Style);
                canvas->drawRRect(sk_rounded_rect(operation.rect, operation.radius), paint);
                break;
            }
            case FISSION_SKIA_FRAME_FILL_PATH:
            case FISSION_SKIA_FRAME_STROKE_PATH: {
                if (operation.kind == FISSION_SKIA_FRAME_STROKE_PATH &&
                    operation.stroke.width == 0.0f) break;
                SkPaint paint;
                if (!configure_paint(frame, operation.paint, &paint) ||
                    (operation.kind == FISSION_SKIA_FRAME_STROKE_PATH &&
                     !configure_stroke(frame, operation.stroke, &paint))) {
                    canvas->restoreToCount(initial_save_count);
                    return fail(FISSION_SKIA_STATUS_INTERNAL, operation_name,
                                "Skia rejected validated path paint", error);
                }
                paint.setStyle(operation.kind == FISSION_SKIA_FRAME_FILL_PATH
                    ? SkPaint::kFill_Style
                    : SkPaint::kStroke_Style);
                canvas->drawPath(
                    sk_path(frame.path_commands + operation.path_offset,
                            operation.path_count, operation.fill_rule),
                    paint);
                break;
            }
            case FISSION_SKIA_FRAME_BOX_SHADOW:
                if (!draw_box_shadow(canvas, operation.rect, operation.radius,
                                     operation.shadow)) {
                    canvas->restoreToCount(initial_save_count);
                    return fail(FISSION_SKIA_STATUS_INVALID_ARGUMENT, operation_name,
                                "box shadow produced invalid derived geometry", error);
                }
                break;
            case FISSION_SKIA_FRAME_DRAW_PARAGRAPH: {
                const auto status = fission_skia_paragraph_draw_picture(
                    fission_skia_paragraph_handle_from_frame_op(operation), canvas,
                    operation.rect.x, operation.rect.y, operation.radius, error);
                if (status != FISSION_SKIA_STATUS_OK) {
                    canvas->restoreToCount(initial_save_count);
                    return status;
                }
                break;
            }
            case FISSION_SKIA_FRAME_DRAW_IMAGE: {
                const auto image = registry().images.find(operation.image.image);
                if (image == registry().images.end()) {
                    canvas->restoreToCount(initial_save_count);
                    return fail(FISSION_SKIA_STATUS_INVALID_HANDLE, operation_name,
                                "image draw handle was destroyed before playback", error);
                }
                const SkSamplingOptions sampling =
                    operation.image.sampling == FISSION_SKIA_IMAGE_SAMPLING_NEAREST
                        ? SkSamplingOptions(SkFilterMode::kNearest,
                                            SkMipmapMode::kNone)
                        : SkSamplingOptions(SkFilterMode::kLinear,
                                            SkMipmapMode::kNone);
                canvas->drawImageRect(
                    image->second->image.get(), sk_rect(operation.image.source),
                    sk_rect(operation.image.destination), sampling, nullptr,
                    SkCanvas::kStrict_SrcRectConstraint);
                break;
            }
            case FISSION_SKIA_FRAME_BACKDROP_BLUR:
                if (surface == nullptr ||
                    !draw_backdrop_blur(*surface, canvas, operation.rect,
                                        operation.radius, operation.sigma)) {
                    canvas->restoreToCount(initial_save_count);
                    return fail(FISSION_SKIA_STATUS_INTERNAL, operation_name,
                                "Skia rejected validated backdrop blur", error);
                }
                break;
            case FISSION_SKIA_FRAME_DRAW_SVG: {
                const auto document =
                    registry().svg_documents.find(operation.svg.document);
                if (document == registry().svg_documents.end()) {
                    canvas->restoreToCount(initial_save_count);
                    return fail(FISSION_SKIA_STATUS_INVALID_HANDLE, operation_name,
                                "SVG document was destroyed before playback", error);
                }
                if (!draw_svg_document(canvas, *document->second,
                                       operation.svg.destination)) {
                    canvas->restoreToCount(initial_save_count);
                    return fail(FISSION_SKIA_STATUS_INTERNAL, operation_name,
                                "Skia rejected validated SVG placement", error);
                }
                break;
            }
            case FISSION_SKIA_FRAME_DRAW_PICTURE: {
                const auto picture = registry().pictures.find(operation.picture.picture);
                if (picture == registry().pictures.end() || !picture->second->picture) {
                    canvas->restoreToCount(initial_save_count);
                    return fail(FISSION_SKIA_STATUS_INVALID_HANDLE, operation_name,
                                "recorded picture was destroyed before playback", error);
                }
                canvas->drawPicture(picture->second->picture);
                break;
            }
        }
    }
    canvas->restoreToCount(initial_save_count);
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
