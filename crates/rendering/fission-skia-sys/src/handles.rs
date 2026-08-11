use std::rc::Rc;

use crate::error::{decode_c_buffer, status_result};
use crate::frame::{Frame, PixelRect};
use crate::thread_affinity::ThreadAffinity;
use crate::{ffi, Error, ErrorKind, Result, ABI_VERSION, SKIA_REVISION};

const REQUIRED_RASTER_FEATURES: u64 = ffi::FEATURE_RASTER_SURFACE
    | ffi::FEATURE_BASIC_FRAME
    | ffi::FEATURE_RGBA_READBACK
    | ffi::FEATURE_STRUCTURED_ERRORS
    | ffi::FEATURE_THREAD_AFFINITY
    | ffi::FEATURE_MEMORY_PRESSURE
    | ffi::FEATURE_PAINT_STATE
    | ffi::FEATURE_OPACITY_LAYER
    | ffi::FEATURE_IMAGE_DECODE
    | ffi::FEATURE_BACKDROP_BLUR
    | ffi::FEATURE_SVG_DOCUMENT
    | ffi::FEATURE_RETAINED_PICTURE;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildInfo {
    pub abi_version: u32,
    pub feature_bits: u64,
    pub skia_revision: String,
    pub profile: String,
}

impl BuildInfo {
    pub fn query() -> Result<Self> {
        let mut info = ffi::AbiInfo::default();
        let mut error = ffi::Error::default();
        // SAFETY: both output pointers are valid for writes for the duration of
        // the call, and their struct_size fields describe the Rust ABI layouts.
        let status = unsafe { ffi::fission_skia_get_abi_info(&mut info, &mut error) };
        status_result(status, &error)?;
        Ok(Self {
            abi_version: info.abi_version,
            feature_bits: info.feature_bits,
            skia_revision: decode_c_buffer(&info.skia_revision),
            profile: decode_c_buffer(&info.build_profile),
        })
    }

    fn validate(&self) -> Result<()> {
        if self.abi_version != ABI_VERSION {
            return Err(Error::local(
                ErrorKind::AbiMismatch,
                "Engine::new",
                format!(
                    "bridge ABI {} does not match required ABI {ABI_VERSION}",
                    self.abi_version
                ),
            ));
        }
        if self.skia_revision != SKIA_REVISION {
            return Err(Error::local(
                ErrorKind::AbiMismatch,
                "Engine::new",
                format!(
                    "bridge Skia revision {} does not match pinned revision {SKIA_REVISION}",
                    self.skia_revision
                ),
            ));
        }
        let missing = REQUIRED_RASTER_FEATURES & !self.feature_bits;
        if missing != 0 {
            return Err(Error::local(
                ErrorKind::Unsupported,
                "Engine::new",
                format!("bridge is missing required raster feature bits 0x{missing:016x}"),
            ));
        }
        Ok(())
    }
}

#[derive(Clone)]
pub struct Engine {
    inner: Rc<EngineInner>,
}

struct EngineInner {
    raw: ffi::EngineHandle,
    thread: ThreadAffinity,
    build_info: BuildInfo,
}

impl Engine {
    pub fn new() -> Result<Self> {
        let build_info = BuildInfo::query()?;
        build_info.validate()?;
        let config = ffi::EngineConfig {
            struct_size: std::mem::size_of::<ffi::EngineConfig>() as u32,
            expected_abi_version: ABI_VERSION,
            required_feature_bits: REQUIRED_RASTER_FEATURES,
        };
        let mut raw = 0;
        let mut error = ffi::Error::default();
        // SAFETY: config and outputs are initialized, properly aligned, and
        // remain alive for the call. A successful call initializes raw.
        let status = unsafe { ffi::fission_skia_engine_create(&config, &mut raw, &mut error) };
        status_result(status, &error)?;
        if raw == 0 {
            return Err(Error::local(
                ErrorKind::Internal,
                "Engine::new",
                "bridge returned a null engine handle after reporting success",
            ));
        }
        Ok(Self {
            inner: Rc::new(EngineInner {
                raw,
                thread: ThreadAffinity::current(),
                build_info,
            }),
        })
    }

    pub fn build_info(&self) -> &BuildInfo {
        &self.inner.build_info
    }

    pub(crate) fn raw_for_owner(&self, operation: &str) -> Result<ffi::EngineHandle> {
        self.inner.thread.ensure_owner(operation)?;
        Ok(self.inner.raw)
    }
}

impl Drop for EngineInner {
    fn drop(&mut self) {
        debug_assert!(
            self.thread.is_owner(),
            "Skia engine dropped off its owner thread"
        );
        let mut error = ffi::Error::default();
        // SAFETY: this is the sole native handle owner. Rc dependencies keep
        // this inner object alive until all contexts have been destroyed.
        let status = unsafe { ffi::fission_skia_engine_destroy(self.raw, &mut error) };
        debug_assert_eq!(
            status,
            ffi::STATUS_OK,
            "native Skia engine destruction failed"
        );
    }
}

#[derive(Clone)]
pub struct Context {
    inner: Rc<ContextInner>,
}

struct ContextInner {
    raw: ffi::ContextHandle,
    thread: ThreadAffinity,
    _engine: Engine,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryPressure {
    Moderate,
    Critical,
}

impl MemoryPressure {
    pub(crate) fn as_ffi(self) -> u32 {
        match self {
            Self::Moderate => ffi::MEMORY_PRESSURE_MODERATE,
            Self::Critical => ffi::MEMORY_PRESSURE_CRITICAL,
        }
    }
}

impl Context {
    pub fn new_raster(engine: &Engine) -> Result<Self> {
        engine.inner.thread.ensure_owner("Context::new_raster")?;
        let mut raw = 0;
        let mut error = ffi::Error::default();
        // SAFETY: engine is a live handle retained by the returned context and
        // the output pointers are valid for the duration of the call.
        let status = unsafe {
            ffi::fission_skia_context_create_raster(engine.inner.raw, &mut raw, &mut error)
        };
        status_result(status, &error)?;
        if raw == 0 {
            return Err(Error::local(
                ErrorKind::Internal,
                "Context::new_raster",
                "bridge returned a null context handle after reporting success",
            ));
        }
        Ok(Self {
            inner: Rc::new(ContextInner {
                raw,
                thread: ThreadAffinity::current(),
                _engine: engine.clone(),
            }),
        })
    }

    pub fn trim_memory(&self, pressure: MemoryPressure) -> Result<()> {
        self.inner.thread.ensure_owner("Context::trim_memory")?;
        let mut error = ffi::Error::default();
        // SAFETY: the context handle is live and owner-thread access was
        // checked immediately before entering the bridge.
        let status = unsafe {
            ffi::fission_skia_context_trim_memory(self.inner.raw, pressure.as_ffi(), &mut error)
        };
        status_result(status, &error)
    }
}

impl Drop for ContextInner {
    fn drop(&mut self) {
        debug_assert!(
            self.thread.is_owner(),
            "Skia context dropped off its owner thread"
        );
        let mut error = ffi::Error::default();
        // SAFETY: this is the sole owner of the native context handle. Surface
        // wrappers retain Context, so no safe child can remain alive here.
        let status = unsafe { ffi::fission_skia_context_destroy(self.raw, &mut error) };
        debug_assert_eq!(
            status,
            ffi::STATUS_OK,
            "native Skia context destruction failed"
        );
    }
}

pub struct RasterSurface {
    raw: ffi::SurfaceHandle,
    width: u32,
    height: u32,
    thread: ThreadAffinity,
    _context: Context,
}

impl RasterSurface {
    pub fn new(context: &Context, width: u32, height: u32) -> Result<Self> {
        context.inner.thread.ensure_owner("RasterSurface::new")?;
        if width == 0 || height == 0 {
            return Err(Error::local(
                ErrorKind::InvalidArgument,
                "RasterSurface::new",
                "raster surface dimensions must be non-zero",
            ));
        }
        let _ = usize::try_from(width)
            .ok()
            .and_then(|width| width.checked_mul(usize::try_from(height).ok()?))
            .and_then(|pixels| pixels.checked_mul(4))
            .ok_or_else(|| {
                Error::local(
                    ErrorKind::InvalidArgument,
                    "RasterSurface::new",
                    "raster surface byte length overflows this platform",
                )
            })?;
        let mut raw = 0;
        let mut error = ffi::Error::default();
        // SAFETY: context is live and retained by the returned surface; output
        // pointers are valid for the duration of the call.
        let status = unsafe {
            ffi::fission_skia_surface_create_raster(
                context.inner.raw,
                width,
                height,
                &mut raw,
                &mut error,
            )
        };
        status_result(status, &error)?;
        if raw == 0 {
            return Err(Error::local(
                ErrorKind::Internal,
                "RasterSurface::new",
                "bridge returned a null surface handle after reporting success",
            ));
        }
        Ok(Self {
            raw,
            width,
            height,
            thread: ThreadAffinity::current(),
            _context: context.clone(),
        })
    }

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }

    pub fn execute_frame(&mut self, frame: &Frame) -> Result<()> {
        self.thread.ensure_owner("RasterSurface::execute_frame")?;
        let encoded = frame.encode()?;
        let raw = encoded.as_raw();
        let mut error = ffi::Error::default();
        // SAFETY: encoded owns both arrays referenced by raw for the complete
        // call and the mutable surface handle has unique safe Rust access.
        let status = unsafe { ffi::fission_skia_surface_execute_frame(self.raw, &raw, &mut error) };
        status_result(status, &error)
    }

    pub fn read_pixels_rgba8888(&self, region: Option<PixelRect>) -> Result<Vec<u8>> {
        self.thread
            .ensure_owner("RasterSurface::read_pixels_rgba8888")?;
        let region = region.unwrap_or(PixelRect::new(0, 0, self.width, self.height));
        self.validate_readback_region(region)?;
        let row_bytes = usize::try_from(region.width)
            .ok()
            .and_then(|width| width.checked_mul(4))
            .ok_or_else(|| readback_overflow("row byte count overflows this platform"))?;
        let length = row_bytes
            .checked_mul(usize::try_from(region.height).unwrap_or(usize::MAX))
            .ok_or_else(|| readback_overflow("pixel byte length overflows this platform"))?;
        let raw_region = ffi::PixelRect {
            x: region.x,
            y: region.y,
            width: region.width,
            height: region.height,
        };
        let mut pixels = vec![0; length];
        let mut required = 0;
        let mut error = ffi::Error::default();
        // SAFETY: the destination allocation is length bytes, the row stride is
        // derived from its width, and raw_region remains alive for the call.
        let status = unsafe {
            ffi::fission_skia_surface_read_pixels_rgba8888(
                self.raw,
                &raw_region,
                pixels.as_mut_ptr(),
                pixels.len(),
                row_bytes,
                &mut required,
                &mut error,
            )
        };
        status_result(status, &error)?;
        if required != length {
            return Err(Error::local(
                ErrorKind::Internal,
                "RasterSurface::read_pixels_rgba8888",
                format!("bridge reported {required} bytes for a {length}-byte readback"),
            ));
        }
        Ok(pixels)
    }

    fn validate_readback_region(&self, region: PixelRect) -> Result<()> {
        if region.width == 0 || region.height == 0 || region.x < 0 || region.y < 0 {
            return Err(Error::local(
                ErrorKind::InvalidArgument,
                "RasterSurface::read_pixels_rgba8888",
                "readback rectangle must be non-empty and have non-negative origin",
            ));
        }
        let right = u32::try_from(region.x)
            .ok()
            .and_then(|x| x.checked_add(region.width));
        let bottom = u32::try_from(region.y)
            .ok()
            .and_then(|y| y.checked_add(region.height));
        if !matches!(right, Some(right) if right <= self.width)
            || !matches!(bottom, Some(bottom) if bottom <= self.height)
        {
            return Err(Error::local(
                ErrorKind::InvalidArgument,
                "RasterSurface::read_pixels_rgba8888",
                "readback rectangle lies outside the raster surface",
            ));
        }
        Ok(())
    }
}

impl Drop for RasterSurface {
    fn drop(&mut self) {
        debug_assert!(
            self.thread.is_owner(),
            "Skia surface dropped off its owner thread"
        );
        let mut error = ffi::Error::default();
        // SAFETY: RasterSurface uniquely owns the native surface handle.
        let status = unsafe { ffi::fission_skia_surface_destroy(self.raw, &mut error) };
        debug_assert_eq!(
            status,
            ffi::STATUS_OK,
            "native Skia surface destruction failed"
        );
    }
}

fn readback_overflow(message: &str) -> Error {
    Error::local(
        ErrorKind::InvalidArgument,
        "RasterSurface::read_pixels_rgba8888",
        message,
    )
}
