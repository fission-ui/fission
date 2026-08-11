#include "fission_skia_internal.h"
#include "fission_skia_paragraph_internal.h"

namespace fission::skia::bridge {

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

}  // namespace fission::skia::bridge
