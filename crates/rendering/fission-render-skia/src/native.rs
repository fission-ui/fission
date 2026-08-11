use fission_render::surface::{MemoryPressure, PhysicalSize};
use fission_skia_sys::{
    Color, Context, Engine, Error, ErrorKind, Frame, FrameOp, PixelRect, RasterSurface, Rect,
};

use crate::api::{
    ApiError, ApiErrorKind, ApiReadback, PixelRegion, RasterCommand, RasterFrame, SkiaApi,
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

    fn execute_frame(
        &self,
        _context: &mut Self::Context,
        surface: &mut Self::Surface,
        frame: &RasterFrame,
    ) -> Result<(), ApiError> {
        let frame = Frame::new(
            frame
                .commands
                .iter()
                .map(|command| match command {
                    RasterCommand::Clear(color) => FrameOp::Clear(native_color(*color)),
                    RasterCommand::FillRect { rect, color } => FrameOp::FillRect {
                        rect: Rect::new(
                            rect.left,
                            rect.top,
                            rect.right - rect.left,
                            rect.bottom - rect.top,
                        ),
                        color: native_color(*color),
                    },
                })
                .collect::<Vec<_>>(),
        );
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

fn native_color(color: crate::api::RasterColor) -> Color {
    Color::rgba(color.red, color.green, color.blue, color.alpha)
}

fn map_error(error: Error) -> ApiError {
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
