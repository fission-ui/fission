//! Maps render fills, gradients, strokes and shapes to the painter's brushes and paths.

use super::*;

pub(crate) fn normalized_point(bounds: Rect, point: (f32, f32)) -> Point {
    Point::new(
        bounds.x0 + bounds.width() * point.0 as f64,
        bounds.y0 + bounds.height() * point.1 as f64,
    )
}

/// Maps the IR's extend mode onto peniko's.
///
/// Vello implements all three, so nothing is lost here.
/// Maps the IR's per-corner radii onto kurbo's.
/// The area of `bounds` outside `shape`: the rectangle wound against the shape, so the shape is a
/// hole under either fill rule.
pub(crate) fn outside_of(shape: &RoundedRect, bounds: Rect) -> BezPath {
    let mut path = shape.to_path(0.1);
    let corners = [
        Point::new(bounds.x0, bounds.y0),
        Point::new(bounds.x1, bounds.y0),
        Point::new(bounds.x1, bounds.y1),
        Point::new(bounds.x0, bounds.y1),
    ];
    let wind = |points: &mut dyn Iterator<Item = &Point>| {
        let mut outer = BezPath::new();
        let mut points = points.peekable();
        if let Some(first) = points.next() {
            outer.move_to(*first);
        }
        for point in points {
            outer.line_to(*point);
        }
        outer.close_path();
        outer
    };
    let mut outer = wind(&mut corners.iter());
    if outer.area().signum() == path.area().signum() {
        outer = wind(&mut corners.iter().rev());
    }
    path.extend(outer);
    path
}

pub(crate) fn kurbo_radii(radii: fission_ir::CornerRadii) -> RoundedRectRadii {
    RoundedRectRadii::new(
        radii.top_left as f64,
        radii.top_right as f64,
        radii.bottom_right as f64,
        radii.bottom_left as f64,
    )
}

pub(crate) fn map_extend(extend: fission_ir::GradientExtend) -> vello_cpu::peniko::Extend {
    match extend {
        fission_ir::GradientExtend::Pad => vello_cpu::peniko::Extend::Pad,
        fission_ir::GradientExtend::Repeat => vello_cpu::peniko::Extend::Repeat,
        fission_ir::GradientExtend::Reflect => vello_cpu::peniko::Extend::Reflect,
    }
}

pub(crate) fn map_fill_to_brush(f: &fission_render::Fill, bounds: Rect) -> PaintType {
    fn gradient_stops<C: Copy>(
        stops: &[(f32, C)],
        to_color: impl Fn(&C) -> Color,
    ) -> Vec<vello_cpu::peniko::ColorStop> {
        stops
            .iter()
            .map(|(offset, color)| vello_cpu::peniko::ColorStop {
                offset: *offset,
                color: to_color(color).into(),
            })
            .collect()
    }

    match f {
        fission_render::Fill::Solid(c) => PaintType::from(map_color(c)),
        fission_render::Fill::LinearGradient {
            start,
            end,
            stops,
            extend,
        } => PaintType::from(
            vello_cpu::peniko::Gradient::new_linear(
                normalized_point(bounds, *start),
                normalized_point(bounds, *end),
            )
            .with_extend(map_extend(*extend))
            .with_stops(gradient_stops(stops, map_color).as_slice()),
        ),
        fission_render::Fill::RadialGradient {
            center,
            radius,
            stops,
            extend,
        } => PaintType::from(
            vello_cpu::peniko::Gradient::new_radial(
                normalized_point(bounds, *center),
                radius * bounds.width().max(bounds.height()) as f32,
            )
            .with_extend(map_extend(*extend))
            .with_stops(gradient_stops(stops, map_color).as_slice()),
        ),
        fission_render::Fill::SweepGradient {
            center,
            start_angle,
            end_angle,
            stops,
            extend,
        } => PaintType::from(
            vello_cpu::peniko::Gradient::new_sweep(
                normalized_point(bounds, *center),
                *start_angle,
                *end_angle,
            )
            .with_extend(map_extend(*extend))
            .with_stops(gradient_stops(stops, map_color).as_slice()),
        ),
    }
}

pub(crate) fn map_text_fill_to_brush(f: &fission_ir::op::Fill, bounds: Rect) -> PaintType {
    fn ir_stops(stops: &[(f32, fission_ir::op::Color)]) -> Vec<vello_cpu::peniko::ColorStop> {
        stops
            .iter()
            .map(|(offset, color)| vello_cpu::peniko::ColorStop {
                offset: *offset,
                color: Color::from_rgba8(color.r, color.g, color.b, color.a).into(),
            })
            .collect()
    }

    match f {
        fission_ir::op::Fill::Solid(c) => PaintType::from(Color::from_rgba8(c.r, c.g, c.b, c.a)),
        fission_ir::op::Fill::LinearGradient {
            start,
            end,
            stops,
            extend,
        } => PaintType::from(
            vello_cpu::peniko::Gradient::new_linear(
                normalized_point(bounds, *start),
                normalized_point(bounds, *end),
            )
            .with_extend(map_extend(*extend))
            .with_stops(ir_stops(stops).as_slice()),
        ),
        fission_ir::op::Fill::RadialGradient {
            center,
            radius,
            stops,
            extend,
        } => PaintType::from(
            vello_cpu::peniko::Gradient::new_radial(
                normalized_point(bounds, *center),
                radius * bounds.width().max(bounds.height()) as f32,
            )
            .with_extend(map_extend(*extend))
            .with_stops(ir_stops(stops).as_slice()),
        ),
        fission_ir::op::Fill::SweepGradient {
            center,
            start_angle,
            end_angle,
            stops,
            extend,
        } => PaintType::from(
            vello_cpu::peniko::Gradient::new_sweep(
                normalized_point(bounds, *center),
                *start_angle,
                *end_angle,
            )
            .with_extend(map_extend(*extend))
            .with_stops(ir_stops(stops).as_slice()),
        ),
    }
}

pub(crate) fn map_stroke(
    s: &fission_render::Stroke,
    bounds: Rect,
) -> (vello_cpu::kurbo::Stroke, PaintType) {
    let cap = match s.line_cap {
        fission_render::LineCap::Butt => vello_cpu::kurbo::Cap::Butt,
        fission_render::LineCap::Round => vello_cpu::kurbo::Cap::Round,
        fission_render::LineCap::Square => vello_cpu::kurbo::Cap::Square,
    };
    let join = match s.line_join {
        fission_render::LineJoin::Miter => vello_cpu::kurbo::Join::Miter,
        fission_render::LineJoin::Round => vello_cpu::kurbo::Join::Round,
        fission_render::LineJoin::Bevel => vello_cpu::kurbo::Join::Bevel,
    };

    let mut stroke = vello_cpu::kurbo::Stroke::new(s.width as f64)
        .with_caps(cap)
        .with_join(join);
    if let Some(dash) = &s.dash_array {
        let dashes: Vec<f64> = dash.iter().map(|v| *v as f64).collect();
        stroke = stroke.with_dashes(s.dash_offset as f64, dashes);
    }

    (stroke, map_fill_to_brush(&s.fill, bounds))
}

/// The path a stroke paints: the whole path, or the part its trim selects.
pub(crate) fn stroke_geometry<'a>(
    path: &str,
    stroke: &fission_render::Stroke,
    parsed: &'a BezPath,
) -> std::borrow::Cow<'a, BezPath> {
    match stroke.trim {
        Some(trim) if !trim.is_full() => fission_ir::path::trim_svg_path(path, trim)
            .map(std::borrow::Cow::Owned)
            .unwrap_or(std::borrow::Cow::Borrowed(parsed)),
        _ => std::borrow::Cow::Borrowed(parsed),
    }
}

/// Scales a path drawn in a `view_box` coordinate space onto its node's bounds.
pub(crate) fn view_box_scale(
    view_box: Option<[f32; 2]>,
    box_width: f32,
    box_height: f32,
) -> Affine {
    match view_box {
        Some([width, height]) if width > 0.0 && height > 0.0 => {
            Affine::scale_non_uniform(f64::from(box_width / width), f64::from(box_height / height))
        }
        _ => Affine::IDENTITY,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vello_cpu::kurbo::Shape;

    fn stroke(trim: Option<fission_ir::op::StrokeTrim>) -> fission_render::Stroke {
        fission_render::Stroke {
            fill: fission_render::Fill::Solid(fission_render::Color {
                r: 0,
                g: 0,
                b: 0,
                a: 255,
            }),
            width: 2.0,
            dash_array: None,
            line_cap: fission_render::LineCap::Butt,
            line_join: fission_render::LineJoin::Miter,
            dash_offset: 0.0,
            trim,
        }
    }

    #[test]
    fn trimmed_strokes_keep_only_the_selected_arc_length() {
        let path = "M0 0 H100";
        let parsed = BezPath::from_svg(path).unwrap();
        let full = stroke_geometry(path, &stroke(None), &parsed);
        assert!(matches!(full, std::borrow::Cow::Borrowed(_)));

        let trim = fission_ir::op::StrokeTrim::new(0.25, 0.75);
        let bounds = stroke_geometry(path, &stroke(Some(trim)), &parsed).bounding_box();
        assert!((bounds.x0 - 25.0).abs() < 0.01, "{bounds:?}");
        assert!((bounds.x1 - 75.0).abs() < 0.01, "{bounds:?}");
    }

    #[test]
    fn view_boxes_scale_to_the_painted_box() {
        assert_eq!(
            view_box_scale(Some([24.0, 24.0]), 48.0, 12.0),
            Affine::scale_non_uniform(2.0, 0.5)
        );
        assert_eq!(view_box_scale(None, 48.0, 12.0), Affine::IDENTITY);
        assert_eq!(
            view_box_scale(Some([0.0, 24.0]), 48.0, 12.0),
            Affine::IDENTITY
        );
    }
}
