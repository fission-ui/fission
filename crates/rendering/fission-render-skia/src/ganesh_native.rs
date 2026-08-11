use fission_render::surface::{MemoryPressure, PhysicalSize};
use fission_skia_sys::{
    Engine, GaneshContext, GaneshSurface, NativeWindow, NativeWindowKind, PixelRect,
    RecordedPicture,
};

use crate::api::{ApiError, ApiErrorKind, ApiReadback, PixelRegion, RasterFrame, RasterRect};
use crate::ganesh_api::{GaneshApi, GaneshResourceCacheUsage};
use crate::native::{map_error, native_frame};

const REQUIRED_NATIVE_GANESH_FEATURES: u64 =
    fission_skia_sys::ffi::FEATURE_GANESH | fission_skia_sys::ffi::FEATURE_NATIVE_PRESENTATION;
#[cfg(target_os = "linux")]
const REQUIRED_GANESH_FEATURES: u64 =
    REQUIRED_NATIVE_GANESH_FEATURES | fission_skia_sys::ffi::FEATURE_VULKAN;
#[cfg(any(target_os = "macos", target_os = "ios"))]
const REQUIRED_GANESH_FEATURES: u64 =
    REQUIRED_NATIVE_GANESH_FEATURES | fission_skia_sys::ffi::FEATURE_METAL;
#[cfg(target_os = "windows")]
const REQUIRED_GANESH_FEATURES: u64 =
    REQUIRED_NATIVE_GANESH_FEATURES | fission_skia_sys::ffi::FEATURE_D3D12;

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
        #[cfg(all(
            not(feature = "test-shim"),
            not(any(
                target_os = "linux",
                target_os = "macos",
                target_os = "ios",
                target_os = "windows"
            ))
        ))]
        {
            return Err(ApiError::new(
                ApiErrorKind::Unsupported,
                "ganesh-platform-unsupported",
                "create_ganesh_engine",
                "native Ganesh supports Linux Vulkan, macOS/iOS Metal, and Windows D3D12",
            ));
        }
        #[cfg(all(
            not(feature = "test-shim"),
            any(
                target_os = "linux",
                target_os = "macos",
                target_os = "ios",
                target_os = "windows"
            )
        ))]
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
        resource_cache_limit_bytes: u64,
    ) -> Result<Self::Context, ApiError> {
        require_platform_window(compatible_window.kind())?;
        #[cfg(target_os = "linux")]
        {
            return GaneshContext::new_vulkan_with_resource_cache_limit(
                engine,
                compatible_window,
                resource_cache_limit_bytes,
            )
            .map_err(map_error);
        }
        #[cfg(any(target_os = "macos", target_os = "ios"))]
        {
            return GaneshContext::new_metal_with_resource_cache_limit(
                engine,
                compatible_window,
                resource_cache_limit_bytes,
            )
            .map_err(map_error);
        }
        #[cfg(target_os = "windows")]
        {
            return GaneshContext::new_d3d12_with_resource_cache_limit(
                engine,
                compatible_window,
                resource_cache_limit_bytes,
            )
            .map_err(map_error);
        }
        #[cfg(not(any(
            target_os = "linux",
            target_os = "macos",
            target_os = "ios",
            target_os = "windows"
        )))]
        {
            let _ = (engine, resource_cache_limit_bytes);
            Err(unsupported_platform_window(compatible_window.kind()))
        }
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

    fn resource_cache_usage(
        &self,
        context: &Self::Context,
    ) -> Result<GaneshResourceCacheUsage, ApiError> {
        context
            .resource_cache_usage()
            .map(|usage| GaneshResourceCacheUsage {
                resource_count: usage.resource_count,
                resource_bytes: usage.resource_bytes,
            })
            .map_err(map_error)
    }
}

fn require_platform_window(kind: NativeWindowKind) -> Result<(), ApiError> {
    let supported = match kind {
        NativeWindowKind::Wayland | NativeWindowKind::Xlib | NativeWindowKind::Xcb => {
            cfg!(target_os = "linux")
        }
        NativeWindowKind::AppKit => cfg!(target_os = "macos"),
        NativeWindowKind::UIKit => cfg!(target_os = "ios"),
        NativeWindowKind::Win32 => cfg!(target_os = "windows"),
    };
    if supported {
        Ok(())
    } else {
        Err(unsupported_platform_window(kind))
    }
}

fn unsupported_platform_window(kind: NativeWindowKind) -> ApiError {
    ApiError::new(
        ApiErrorKind::InvalidArgument,
        "ganesh-window-platform-mismatch",
        "create_ganesh_context",
        format!("native window kind {kind:?} does not match this target platform"),
    )
}

fn native_rect(rect: RasterRect) -> fission_skia_sys::Rect {
    fission_skia_sys::Rect::new(
        rect.left,
        rect.top,
        rect.right - rect.left,
        rect.bottom - rect.top,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_context_accepts_only_the_current_platform_window_family() {
        #[cfg(target_os = "linux")]
        {
            assert!(require_platform_window(NativeWindowKind::Wayland).is_ok());
            assert!(require_platform_window(NativeWindowKind::Xlib).is_ok());
            assert!(require_platform_window(NativeWindowKind::Xcb).is_ok());
            assert!(require_platform_window(NativeWindowKind::AppKit).is_err());
            assert!(require_platform_window(NativeWindowKind::UIKit).is_err());
            assert!(require_platform_window(NativeWindowKind::Win32).is_err());
        }
        #[cfg(target_os = "macos")]
        {
            assert!(require_platform_window(NativeWindowKind::AppKit).is_ok());
            assert!(require_platform_window(NativeWindowKind::UIKit).is_err());
            assert!(require_platform_window(NativeWindowKind::Xlib).is_err());
            assert!(require_platform_window(NativeWindowKind::Win32).is_err());
        }
        #[cfg(target_os = "ios")]
        {
            assert!(require_platform_window(NativeWindowKind::UIKit).is_ok());
            assert!(require_platform_window(NativeWindowKind::AppKit).is_err());
            assert!(require_platform_window(NativeWindowKind::Xlib).is_err());
            assert!(require_platform_window(NativeWindowKind::Win32).is_err());
        }
        #[cfg(target_os = "windows")]
        {
            assert!(require_platform_window(NativeWindowKind::Win32).is_ok());
            assert!(require_platform_window(NativeWindowKind::Xlib).is_err());
            assert!(require_platform_window(NativeWindowKind::AppKit).is_err());
        }
        #[cfg(not(any(
            target_os = "linux",
            target_os = "macos",
            target_os = "ios",
            target_os = "windows"
        )))]
        for kind in [
            NativeWindowKind::Wayland,
            NativeWindowKind::Xlib,
            NativeWindowKind::Xcb,
            NativeWindowKind::AppKit,
            NativeWindowKind::UIKit,
            NativeWindowKind::Win32,
        ] {
            assert!(require_platform_window(kind).is_err());
        }
    }
}
