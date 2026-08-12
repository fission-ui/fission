use fission_render::surface::{MemoryPressure, PhysicalSize};
use fission_skia_sys::{
    Affine, BoxShadow, Color, Context, Engine, Error, ErrorKind, FillRule, Frame, FrameOp,
    GradientStop, ImageSampling, LineCap, LineJoin, Paint, Path, PathCommand, PixelRect, Point,
    RasterSurface, Rect, Stroke,
};

use crate::api::{
    ApiError, ApiErrorKind, ApiReadback, PixelRegion, RasterAffine, RasterBoxShadow, RasterCommand,
    RasterFillRule, RasterFrame, RasterLineCap, RasterLineJoin, RasterPaint, RasterPath,
    RasterPathCommand, RasterPoint, RasterStroke, SkiaApi,
};

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct NativeSkiaApi;

impl SkiaApi for NativeSkiaApi {
    type Engine = Engine;
    type Context = Context;
    type Surface = RasterSurface;

    fn create_engine(&self) -> Result<Self::Engine, ApiError> {
        #[cfg(feature = "test-shim")]
        {
            return Err(ApiError::new(
                ApiErrorKind::Unsupported,
                "test-shim-is-not-a-renderer",
                "create_engine",
                "the Skia ABI test shim cannot back a Fission renderer session",
            ));
        }
        #[cfg(not(feature = "test-shim"))]
        Engine::new().map_err(map_error)
    }

    fn create_raster_context(&self, engine: &Self::Engine) -> Result<Self::Context, ApiError> {
        Context::new_raster(engine).map_err(map_error)
    }

    fn create_raster_surface(
        &self,
        context: &Self::Context,
        size: PhysicalSize,
    ) -> Result<Self::Surface, ApiError> {
        RasterSurface::new(context, size.width, size.height).map_err(map_error)
    }

    fn record_picture(
        &self,
        bounds: crate::api::RasterRect,
        frame: &RasterFrame,
    ) -> Result<Option<fission_skia_sys::RecordedPicture>, ApiError> {
        let frame = native_frame(frame)?;
        fission_skia_sys::RecordedPicture::record(native_rect(bounds), &frame)
            .map(Some)
            .map_err(map_error)
    }

    fn execute_frame(
        &self,
        _context: &mut Self::Context,
        surface: &mut Self::Surface,
        frame: &RasterFrame,
    ) -> Result<(), ApiError> {
        let frame = native_frame(frame)?;
        surface.execute_frame(&frame).map_err(map_error)
    }

    fn read_pixels_rgba8888(
        &self,
        _context: &mut Self::Context,
        surface: &mut Self::Surface,
        region: PixelRegion,
    ) -> Result<ApiReadback, ApiError> {
        let x = i32::try_from(region.x).map_err(|_| {
            ApiError::new(
                ApiErrorKind::InvalidArgument,
                "pixel-origin-overflow",
                "read_pixels_rgba8888",
                "readback x origin exceeds the native ABI range",
            )
        })?;
        let y = i32::try_from(region.y).map_err(|_| {
            ApiError::new(
                ApiErrorKind::InvalidArgument,
                "pixel-origin-overflow",
                "read_pixels_rgba8888",
                "readback y origin exceeds the native ABI range",
            )
        })?;
        let pixels = surface
            .read_pixels_rgba8888(Some(PixelRect::new(x, y, region.width, region.height)))
            .map_err(map_error)?;
        let row_bytes = usize::try_from(region.width)
            .ok()
            .and_then(|width| width.checked_mul(4))
            .ok_or_else(|| {
                ApiError::new(
                    ApiErrorKind::InvalidArgument,
                    "readback-size-overflow",
                    "read_pixels_rgba8888",
                    "readback row byte count overflows this platform",
                )
            })?;
        Ok(ApiReadback {
            size: region.size(),
            row_bytes,
            pixels,
        })
    }

    fn trim_memory(
        &self,
        context: &mut Self::Context,
        pressure: MemoryPressure,
    ) -> Result<(), ApiError> {
        let pressure = match pressure {
            MemoryPressure::Moderate => fission_skia_sys::MemoryPressure::Moderate,
            MemoryPressure::Critical => fission_skia_sys::MemoryPressure::Critical,
        };
        context.trim_memory(pressure).map_err(map_error)
    }
}

pub(crate) fn native_frame(frame: &RasterFrame) -> Result<Frame, ApiError> {
    Ok(Frame::new(
        frame
            .commands
            .iter()
            .map(native_command)
            .collect::<Result<Vec<_>, _>>()?,
    ))
}

fn native_command(command: &RasterCommand) -> Result<FrameOp, ApiError> {
    Ok(match command {
        RasterCommand::Clear(color) => FrameOp::Clear(native_color(*color)),
        RasterCommand::Save => FrameOp::Save,
        RasterCommand::Restore => FrameOp::Restore,
        RasterCommand::OpacityLayer { bounds, alpha } => FrameOp::OpacityLayer {
            bounds: native_rect(*bounds),
            alpha: *alpha,
        },
        RasterCommand::BackdropBlur {
            bounds,
            corner_radius,
            sigma,
        } => FrameOp::BackdropBlur {
            bounds: native_rect(*bounds),
            corner_radius: *corner_radius,
            sigma: *sigma,
        },
        RasterCommand::ClipRect { rect } => FrameOp::ClipRect {
            rect: native_rect(*rect),
        },
        RasterCommand::ClipRoundedRect { rect, radius } => FrameOp::ClipRoundedRect {
            rect: native_rect(*rect),
            radius: *radius,
        },
        RasterCommand::ConcatAffine(affine) => FrameOp::ConcatAffine(native_affine(*affine)),
        RasterCommand::FillRect {
            rect,
            radius,
            paint,
        } => FrameOp::FillRect {
            rect: native_rect(*rect),
            radius: *radius,
            paint: native_paint(paint),
        },
        RasterCommand::StrokeRect {
            rect,
            radius,
            stroke,
        } => FrameOp::StrokeRect {
            rect: native_rect(*rect),
            radius: *radius,
            stroke: native_stroke(stroke),
        },
        RasterCommand::FillPath { path, paint } => FrameOp::FillPath {
            path: native_path(path),
            paint: native_paint(paint),
        },
        RasterCommand::StrokePath { path, stroke } => FrameOp::StrokePath {
            path: native_path(path),
            stroke: native_stroke(stroke),
        },
        RasterCommand::BoxShadow {
            rect,
            radius,
            shadow,
        } => FrameOp::BoxShadow {
            rect: native_rect(*rect),
            radius: *radius,
            shadow: native_shadow(*shadow),
        },
        RasterCommand::DrawParagraph {
            data,
            origin,
            scale_factor,
        } => FrameOp::DrawParagraph {
            data: data.as_ref().clone(),
            origin: native_point(*origin),
            scale_factor: *scale_factor,
        },
        RasterCommand::DrawImage {
            image,
            source,
            destination,
        } => FrameOp::DrawImage {
            image: image.clone(),
            source: native_rect(*source),
            destination: native_rect(*destination),
            sampling: ImageSampling::Linear,
        },
        RasterCommand::DrawImageResource { .. } => {
            return Err(ApiError::new(
                ApiErrorKind::Unsupported,
                "web-image-resource-on-native",
                "native_frame",
                "a browser-owned image resource escaped into native Skia execution",
            ))
        }
        RasterCommand::DrawSvg {
            document,
            destination,
        } => FrameOp::DrawSvg {
            document: document.clone(),
            destination: native_rect(*destination),
        },
        RasterCommand::DrawPicture { picture } => FrameOp::DrawPicture {
            picture: picture.clone(),
        },
    })
}

fn native_color(color: crate::api::RasterColor) -> Color {
    Color::rgba(color.red, color.green, color.blue, color.alpha)
}

fn native_point(point: RasterPoint) -> Point {
    Point::new(point.x, point.y)
}

fn native_rect(rect: crate::api::RasterRect) -> Rect {
    Rect::new(
        rect.left,
        rect.top,
        rect.right - rect.left,
        rect.bottom - rect.top,
    )
}

fn native_affine(affine: RasterAffine) -> Affine {
    Affine {
        scale_x: affine.scale_x,
        skew_x: affine.skew_x,
        translate_x: affine.translate_x,
        skew_y: affine.skew_y,
        scale_y: affine.scale_y,
        translate_y: affine.translate_y,
    }
}

fn native_paint(paint: &RasterPaint) -> Paint {
    match paint {
        RasterPaint::Solid(color) => Paint::Solid(native_color(*color)),
        RasterPaint::LinearGradient { start, end, stops } => Paint::LinearGradient {
            start: native_point(*start),
            end: native_point(*end),
            stops: stops
                .iter()
                .map(|stop| GradientStop::new(stop.offset, native_color(stop.color)))
                .collect(),
        },
        RasterPaint::RadialGradient {
            center,
            radius,
            stops,
        } => Paint::RadialGradient {
            center: native_point(*center),
            radius: *radius,
            stops: stops
                .iter()
                .map(|stop| GradientStop::new(stop.offset, native_color(stop.color)))
                .collect(),
        },
    }
}

fn native_stroke(stroke: &RasterStroke) -> Stroke {
    Stroke {
        paint: native_paint(&stroke.paint),
        width: stroke.width,
        dash_array: stroke.dash_array.clone(),
        line_cap: match stroke.line_cap {
            RasterLineCap::Butt => LineCap::Butt,
            RasterLineCap::Round => LineCap::Round,
            RasterLineCap::Square => LineCap::Square,
        },
        line_join: match stroke.line_join {
            RasterLineJoin::Miter => LineJoin::Miter,
            RasterLineJoin::Round => LineJoin::Round,
            RasterLineJoin::Bevel => LineJoin::Bevel,
        },
    }
}

fn native_path(path: &RasterPath) -> Path {
    Path::new(
        match path.fill_rule {
            RasterFillRule::NonZero => FillRule::NonZero,
            RasterFillRule::EvenOdd => FillRule::EvenOdd,
        },
        path.commands
            .iter()
            .map(|command| match command {
                RasterPathCommand::MoveTo { x, y } => PathCommand::MoveTo { x: *x, y: *y },
                RasterPathCommand::LineTo { x, y } => PathCommand::LineTo { x: *x, y: *y },
                RasterPathCommand::QuadTo { cx, cy, x, y } => PathCommand::QuadTo {
                    cx: *cx,
                    cy: *cy,
                    x: *x,
                    y: *y,
                },
                RasterPathCommand::CubicTo {
                    c1x,
                    c1y,
                    c2x,
                    c2y,
                    x,
                    y,
                } => PathCommand::CubicTo {
                    c1x: *c1x,
                    c1y: *c1y,
                    c2x: *c2x,
                    c2y: *c2y,
                    x: *x,
                    y: *y,
                },
                RasterPathCommand::Close => PathCommand::Close,
            })
            .collect::<Vec<_>>(),
    )
}

fn native_shadow(shadow: RasterBoxShadow) -> BoxShadow {
    BoxShadow {
        color: native_color(shadow.color),
        blur_radius: shadow.blur_radius,
        spread_radius: shadow.spread_radius,
        offset: native_point(shadow.offset),
        inset: shadow.inset,
    }
}

pub(crate) fn map_error(error: Error) -> ApiError {
    let (kind, code) = match error.kind {
        ErrorKind::InvalidArgument => (ApiErrorKind::InvalidArgument, "invalid-argument"),
        ErrorKind::InvalidHandle => (ApiErrorKind::Internal, "invalid-handle"),
        ErrorKind::InvalidState => (ApiErrorKind::Internal, "invalid-state"),
        ErrorKind::Unsupported => (ApiErrorKind::Unsupported, "unsupported"),
        ErrorKind::WrongThread => (ApiErrorKind::WrongThread, "wrong-thread"),
        ErrorKind::SurfaceLost => (ApiErrorKind::SurfaceLost, "surface-lost"),
        ErrorKind::ContextLost | ErrorKind::DeviceLost => (ApiErrorKind::DeviceLost, "device-lost"),
        ErrorKind::OutOfMemory => (ApiErrorKind::OutOfMemory, "out-of-memory"),
        ErrorKind::AbiMismatch => (ApiErrorKind::Internal, "abi-mismatch"),
        ErrorKind::Internal => (ApiErrorKind::Internal, "internal"),
        ErrorKind::Unknown(_) => (ApiErrorKind::Internal, "unknown"),
        _ => (ApiErrorKind::Internal, "unknown"),
    };
    ApiError::new(
        kind,
        format!("{code}:{}", error.sequence),
        error.operation,
        error.message,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backdrop_blur_mapping_is_atomic_and_preserves_physical_values() {
        let command = RasterCommand::BackdropBlur {
            bounds: crate::api::RasterRect {
                left: 2.0,
                top: 4.0,
                right: 22.0,
                bottom: 36.0,
            },
            corner_radius: 6.0,
            sigma: 8.0,
        };

        assert_eq!(
            native_command(&command),
            Ok(FrameOp::BackdropBlur {
                bounds: Rect::new(2.0, 4.0, 20.0, 32.0),
                corner_radius: 6.0,
                sigma: 8.0,
            })
        );
    }

    #[cfg(feature = "test-shim")]
    #[test]
    fn svg_mapping_pins_the_document_and_preserves_the_destination() {
        let content = b"<svg viewBox='0 0 2 1'><rect width='2' height='1'/></svg>";
        let document = fission_skia_sys::SvgDocument::parse(content).unwrap();
        let destination = crate::api::RasterRect {
            left: 2.0,
            top: 4.0,
            right: 22.0,
            bottom: 14.0,
        };

        assert_eq!(
            native_command(&RasterCommand::DrawSvg {
                document: document.clone(),
                destination,
            }),
            Ok(FrameOp::DrawSvg {
                document,
                destination: Rect::new(2.0, 4.0, 20.0, 10.0),
            })
        );
    }
}
