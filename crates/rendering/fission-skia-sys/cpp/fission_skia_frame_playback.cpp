#include "fission_skia_internal.h"
#include "fission_skia_paragraph_internal.h"

#include "include/core/SkCanvas.h"
#include "include/core/SkColorSpace.h"
#include "include/core/SkMaskFilter.h"
#include "include/core/SkMatrix.h"
#include "include/core/SkPaint.h"
#include "include/core/SkPathBuilder.h"
#include "include/core/SkRRect.h"
#include "include/core/SkSamplingOptions.h"
#include "include/effects/SkDashPathEffect.h"
#include "include/effects/SkGradient.h"
#include "include/effects/SkImageFilters.h"

#include <algorithm>
#include <cmath>
#include <limits>
#include <vector>

namespace fission::skia::bridge {

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

}  // namespace fission::skia::bridge
