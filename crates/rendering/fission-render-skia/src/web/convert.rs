use fission_skia_sys::web::WebCommand;
use fission_skia_sys::{
    Affine, BoxShadow, Color, FillRule, GradientStop, LineCap, LineJoin, Paint, Path, PathCommand,
    Point, Rect, Stroke,
};

use super::WebCompileError;
use crate::api::{
    RasterAffine, RasterBoxShadow, RasterColor, RasterCommand, RasterFillRule, RasterGradientStop,
    RasterLineCap, RasterLineJoin, RasterPaint, RasterPath, RasterPathCommand, RasterPoint,
    RasterRect, RasterStroke,
};

pub(super) fn web_command(command: &RasterCommand) -> Result<WebCommand, WebCompileError> {
    Ok(match command {
        RasterCommand::Clear(value) => WebCommand::Clear(color(*value)),
        RasterCommand::Save => WebCommand::Save,
        RasterCommand::Restore => WebCommand::Restore,
        RasterCommand::OpacityLayer { bounds, alpha } => WebCommand::OpacityLayer {
            bounds: rect(*bounds)?,
            alpha: *alpha,
        },
        RasterCommand::BackdropBlur {
            bounds,
            corner_radius,
            sigma,
        } => WebCommand::BackdropBlur {
            bounds: rect(*bounds)?,
            corner_radius: *corner_radius,
            sigma: *sigma,
        },
        RasterCommand::ClipRect { rect: value } => WebCommand::ClipRect(rect(*value)?),
        RasterCommand::ClipRoundedRect {
            rect: value,
            radius,
        } => WebCommand::ClipRoundedRect {
            rect: rect(*value)?,
            radius: *radius,
        },
        RasterCommand::ConcatAffine(value) => WebCommand::ConcatAffine(affine(*value)),
        RasterCommand::FillRect {
            rect: value,
            radius,
            paint: value_paint,
        } => WebCommand::FillRect {
            rect: rect(*value)?,
            radius: *radius,
            paint: paint(value_paint),
        },
        RasterCommand::StrokeRect {
            rect: value,
            radius,
            stroke: value_stroke,
        } => WebCommand::StrokeRect {
            rect: rect(*value)?,
            radius: *radius,
            stroke: stroke(value_stroke),
        },
        RasterCommand::FillPath {
            path: value_path,
            paint: value_paint,
        } => WebCommand::FillPath {
            path: path(value_path),
            paint: paint(value_paint),
        },
        RasterCommand::StrokePath {
            path: value_path,
            stroke: value_stroke,
        } => WebCommand::StrokePath {
            path: path(value_path),
            stroke: stroke(value_stroke),
        },
        RasterCommand::BoxShadow {
            rect: value,
            radius,
            shadow: value_shadow,
        } => WebCommand::BoxShadow {
            rect: rect(*value)?,
            radius: *radius,
            shadow: shadow(*value_shadow),
        },
        RasterCommand::DrawParagraph { .. } => {
            return Err(WebCompileError::NativeResource("paragraph"))
        }
        RasterCommand::DrawImage { .. } => return Err(WebCompileError::NativeResource("image")),
        RasterCommand::DrawSvg { .. } => {
            return Err(WebCompileError::NativeResource("SVG document"))
        }
        RasterCommand::DrawPicture { .. } => {
            return Err(WebCompileError::NativeResource("retained picture"))
        }
    })
}

fn color(value: RasterColor) -> Color {
    Color::rgba(value.red, value.green, value.blue, value.alpha)
}

fn point(value: RasterPoint) -> Point {
    Point::new(value.x, value.y)
}

fn rect(value: RasterRect) -> Result<Rect, WebCompileError> {
    let width = value.right - value.left;
    let height = value.bottom - value.top;
    if ![
        value.left,
        value.top,
        value.right,
        value.bottom,
        width,
        height,
    ]
    .iter()
    .all(|coordinate| coordinate.is_finite())
        || width < 0.0
        || height < 0.0
    {
        return Err(WebCompileError::InvalidGeometry("rectangle"));
    }
    Ok(Rect::new(value.left, value.top, width, height))
}

fn affine(value: RasterAffine) -> Affine {
    Affine {
        scale_x: value.scale_x,
        skew_x: value.skew_x,
        translate_x: value.translate_x,
        skew_y: value.skew_y,
        scale_y: value.scale_y,
        translate_y: value.translate_y,
    }
}

fn paint(value: &RasterPaint) -> Paint {
    match value {
        RasterPaint::Solid(value) => Paint::Solid(color(*value)),
        RasterPaint::LinearGradient { start, end, stops } => Paint::LinearGradient {
            start: point(*start),
            end: point(*end),
            stops: gradient_stops(stops),
        },
        RasterPaint::RadialGradient {
            center,
            radius,
            stops,
        } => Paint::RadialGradient {
            center: point(*center),
            radius: *radius,
            stops: gradient_stops(stops),
        },
    }
}

fn gradient_stops(values: &[RasterGradientStop]) -> Vec<GradientStop> {
    values
        .iter()
        .map(|stop| GradientStop::new(stop.offset, color(stop.color)))
        .collect()
}

fn stroke(value: &RasterStroke) -> Stroke {
    Stroke {
        paint: paint(&value.paint),
        width: value.width,
        dash_array: value.dash_array.clone(),
        line_cap: match value.line_cap {
            RasterLineCap::Butt => LineCap::Butt,
            RasterLineCap::Round => LineCap::Round,
            RasterLineCap::Square => LineCap::Square,
        },
        line_join: match value.line_join {
            RasterLineJoin::Miter => LineJoin::Miter,
            RasterLineJoin::Round => LineJoin::Round,
            RasterLineJoin::Bevel => LineJoin::Bevel,
        },
    }
}

fn path(value: &RasterPath) -> Path {
    Path::new(
        match value.fill_rule {
            RasterFillRule::NonZero => FillRule::NonZero,
            RasterFillRule::EvenOdd => FillRule::EvenOdd,
        },
        value
            .commands
            .iter()
            .map(|command| match *command {
                RasterPathCommand::MoveTo { x, y } => PathCommand::MoveTo { x, y },
                RasterPathCommand::LineTo { x, y } => PathCommand::LineTo { x, y },
                RasterPathCommand::QuadTo { cx, cy, x, y } => PathCommand::QuadTo { cx, cy, x, y },
                RasterPathCommand::CubicTo {
                    c1x,
                    c1y,
                    c2x,
                    c2y,
                    x,
                    y,
                } => PathCommand::CubicTo {
                    c1x,
                    c1y,
                    c2x,
                    c2y,
                    x,
                    y,
                },
                RasterPathCommand::Close => PathCommand::Close,
            })
            .collect::<Vec<_>>(),
    )
}

fn shadow(value: RasterBoxShadow) -> BoxShadow {
    BoxShadow {
        color: color(value.color),
        blur_radius: value.blur_radius,
        spread_radius: value.spread_radius,
        offset: point(value.offset),
        inset: value.inset,
    }
}
