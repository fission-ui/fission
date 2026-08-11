use fission_render::surface::{MemoryPressure, PhysicalSize};
use fission_skia_sys::{
    Engine, GaneshContext, GaneshSurface, NativeWindow, PixelRect, RecordedPicture,
};

use crate::api::{ApiError, ApiErrorKind, ApiReadback, PixelRegion, RasterFrame, RasterRect};
use crate::ganesh_api::GaneshApi;
use crate::native::{map_error, native_frame};

const REQUIRED_GANESH_FEATURES: u64 = fission_skia_sys::ffi::FEATURE_GANESH
    | fission_skia_sys::ffi::FEATURE_VULKAN
    | fission_skia_sys::ffi::FEATURE_NATIVE_PRESENTATION;

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct NativeGaneshApi;

impl GaneshApi for NativeGaneshApi {
    type Engine = Engine;
    type Context = GaneshContext;
    type Surface = GaneshSurface;

    fn create_engine(&self) -> Result<Self::Engine, ApiError> {
        #[cfg(feature = "test-shim")]
        {
            return Err(ApiError::new(
                ApiErrorKind::Unsupported,
                "test-shim-is-not-a-renderer",
                "create_ganesh_engine",
                "the Skia ABI test shim cannot back a Ganesh renderer session",
            ));
        }
        #[cfg(not(target_os = "linux"))]
        {
            return Err(ApiError::new(
                ApiErrorKind::Unsupported,
                "ganesh-platform-unsupported",
                "create_ganesh_engine",
                "the first native Ganesh profile supports Linux Vulkan only",
            ));
        }
        #[cfg(all(not(feature = "test-shim"), target_os = "linux"))]
        {
            let engine = Engine::new().map_err(map_error)?;
            let missing = REQUIRED_GANESH_FEATURES & !engine.build_info().feature_bits;
            if missing != 0 {
                return Err(ApiError::new(
                    ApiErrorKind::Unsupported,
                    "ganesh-features-missing",
                    "create_ganesh_engine",
                    format!(
                        "bridge does not advertise required Ganesh feature bits 0x{missing:016x}"
                    ),
                ));
            }
            Ok(engine)
        }
    }

    fn create_context(
        &self,
        engine: &Self::Engine,
        compatible_window: NativeWindow,
    ) -> Result<Self::Context, ApiError> {
        GaneshContext::new_vulkan(engine, compatible_window).map_err(map_error)
    }

    fn create_surface(
        &self,
        context: &Self::Context,
        window: NativeWindow,
        size: PhysicalSize,
    ) -> Result<Self::Surface, ApiError> {
        GaneshSurface::new(context, window, size.width, size.height).map_err(map_error)
    }

    fn resize_surface(
        &self,
        surface: &mut Self::Surface,
        window: NativeWindow,
        size: PhysicalSize,
    ) -> Result<(), ApiError> {
        surface
            .resize(window, size.width, size.height)
            .map_err(map_error)
    }

    fn record_picture(
        &self,
        bounds: RasterRect,
        frame: &RasterFrame,
    ) -> Result<Option<RecordedPicture>, ApiError> {
        let frame = native_frame(frame);
        RecordedPicture::record(native_rect(bounds), &frame)
            .map(Some)
            .map_err(map_error)
    }

    fn execute_frame(
        &self,
        surface: &mut Self::Surface,
        frame: &RasterFrame,
    ) -> Result<(), ApiError> {
        let frame = native_frame(frame);
        surface.execute_frame(&frame).map_err(map_error)
    }

    fn read_pixels_rgba8888(
        &self,
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

    fn present(&self, surface: &mut Self::Surface) -> Result<(), ApiError> {
        surface.present().map_err(map_error)
    }

    fn trim_memory(
        &self,
        context: &Self::Context,
        pressure: MemoryPressure,
    ) -> Result<(), ApiError> {
        let pressure = match pressure {
            MemoryPressure::Moderate => fission_skia_sys::MemoryPressure::Moderate,
            MemoryPressure::Critical => fission_skia_sys::MemoryPressure::Critical,
        };
        context.trim_memory(pressure).map_err(map_error)
    }
}

fn native_rect(rect: RasterRect) -> fission_skia_sys::Rect {
    fission_skia_sys::Rect::new(
        rect.left,
        rect.top,
        rect.right - rect.left,
        rect.bottom - rect.top,
    )
}
