use fission_render::TextStyle as RenderTextStyle;
use vello::kurbo::{Point, Rect};
use vello::peniko::{Brush, Color};

use crate::text;

pub(crate) fn text_style_requires_rich_layout(style: &RenderTextStyle) -> bool {
    text::text_style_requires_rich_layout(style)
}

pub(crate) fn map_color(c: &fission_render::Color) -> Color {
    Color::from_rgba8(c.r, c.g, c.b, c.a).into()
}

fn normalized_point(bounds: Rect, point: (f32, f32)) -> Point {
    Point::new(
        bounds.x0 + bounds.width() * point.0 as f64,
        bounds.y0 + bounds.height() * point.1 as f64,
    )
}

pub(crate) fn map_fill_to_brush(f: &fission_render::Fill, bounds: Rect) -> Brush {
    match f {
        fission_render::Fill::Solid(c) => Brush::Solid(map_color(c)),
        fission_render::Fill::LinearGradient { start, end, stops } => {
            let vello_stops: Vec<_> = stops
                .iter()
                .map(|(o, c)| vello::peniko::ColorStop {
                    offset: *o,
                    color: map_color(c).into(),
                })
                .collect();
            Brush::Gradient(
                vello::peniko::Gradient::new_linear(
                    normalized_point(bounds, *start),
                    normalized_point(bounds, *end),
                )
                .with_stops(vello_stops.as_slice()),
            )
        }
        fission_render::Fill::RadialGradient {
            center,
            radius,
            stops,
        } => {
            let vello_stops: Vec<_> = stops
                .iter()
                .map(|(o, c)| vello::peniko::ColorStop {
                    offset: *o,
                    color: map_color(c).into(),
                })
                .collect();
            Brush::Gradient(
                vello::peniko::Gradient::new_radial(
                    normalized_point(bounds, *center),
                    radius * bounds.width().max(bounds.height()) as f32,
                )
                .with_stops(vello_stops.as_slice()),
            )
        }
    }
}

pub(crate) fn map_stroke(
    s: &fission_render::Stroke,
    bounds: Rect,
) -> (vello::kurbo::Stroke, Brush) {
    let cap = match s.line_cap {
        fission_render::LineCap::Butt => vello::kurbo::Cap::Butt,
        fission_render::LineCap::Round => vello::kurbo::Cap::Round,
        fission_render::LineCap::Square => vello::kurbo::Cap::Square,
    };
    let join = match s.line_join {
        fission_render::LineJoin::Miter => vello::kurbo::Join::Miter,
        fission_render::LineJoin::Round => vello::kurbo::Join::Round,
        fission_render::LineJoin::Bevel => vello::kurbo::Join::Bevel,
    };

    let mut stroke = vello::kurbo::Stroke::new(s.width as f64)
        .with_caps(cap)
        .with_join(join);
    if let Some(dash) = &s.dash_array {
        let dashes: Vec<f64> = dash.iter().map(|v| *v as f64).collect();
        stroke = stroke.with_dashes(0.0, dashes);
    }

    (stroke, map_fill_to_brush(&s.fill, bounds))
}
