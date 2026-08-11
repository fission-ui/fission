use std::ffi::c_void;
use std::num::{NonZeroU32, NonZeroU64};
use std::ptr::NonNull;
use std::rc::Rc;

use crate::error::status_result;
use crate::thread_affinity::ThreadAffinity;
use crate::{ffi, Engine, Error, ErrorKind, Frame, MemoryPressure, Result};

const REQUIRED_FEATURES: u64 =
    ffi::FEATURE_GANESH | ffi::FEATURE_VULKAN | ffi::FEATURE_NATIVE_PRESENTATION;

/// Linux window-system route used by a Vulkan presentation surface.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeWindowKind {
    Wayland,
    Xlib,
    Xcb,
}

/// Fixed-width native window descriptor borrowed by the Skia bridge.
///
/// This value does not own the display connection or window. Its constructors
/// are unsafe because only the platform shell can prove that the supplied
/// native handles have the advertised type and remain live while the bridge
/// uses them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NativeWindow {
    raw: ffi::NativeWindow,
}

impl NativeWindow {
    /// Describes a Wayland display and surface.
    ///
    /// # Safety
    ///
    /// `display` must point to a live `wl_display` and `surface` to a live
    /// `wl_surface` belonging to that display. They must remain valid through
    /// the bridge call that receives this descriptor. When used for surface
    /// creation or resize, they must then remain valid for that attachment
    /// until it is replaced by another resize or the surface is dropped.
    pub unsafe fn wayland(display: NonNull<c_void>, surface: NonNull<c_void>) -> Self {
        Self::new(
            ffi::NATIVE_WINDOW_WAYLAND,
            pointer_bits(display),
            pointer_bits(surface),
            0,
        )
    }

    /// Describes an Xlib display, window, and visual.
    ///
    /// # Safety
    ///
    /// `display` must point to a live Xlib `Display`, and `window` must identify
    /// a live window on it. `visual_id` is optional metadata and may be zero.
    /// The native objects must remain valid under the same rules as
    /// [`Self::wayland`].
    pub unsafe fn xlib(display: NonNull<c_void>, window: NonZeroU64, visual_id: u64) -> Self {
        Self::new(
            ffi::NATIVE_WINDOW_XLIB,
            pointer_bits(display),
            window.get(),
            visual_id,
        )
    }

    /// Describes an XCB connection, window, and visual.
    ///
    /// # Safety
    ///
    /// `connection` must point to a live `xcb_connection_t`, and `window` must
    /// identify a live window on that connection. `visual_id` is optional
    /// metadata and may be zero. The native objects must remain valid under the
    /// same rules as [`Self::wayland`].
    pub unsafe fn xcb(connection: NonNull<c_void>, window: NonZeroU32, visual_id: u32) -> Self {
        Self::new(
            ffi::NATIVE_WINDOW_XCB,
            pointer_bits(connection),
            u64::from(window.get()),
            u64::from(visual_id),
        )
    }

    pub fn kind(self) -> NativeWindowKind {
        match self.raw.kind {
            ffi::NATIVE_WINDOW_WAYLAND => NativeWindowKind::Wayland,
            ffi::NATIVE_WINDOW_XLIB => NativeWindowKind::Xlib,
            ffi::NATIVE_WINDOW_XCB => NativeWindowKind::Xcb,
            _ => unreachable!("safe NativeWindow contains an unknown kind"),
        }
    }

    fn new(kind: u32, display: u64, window: u64, visual_id: u64) -> Self {
        Self {
            raw: ffi::NativeWindow {
                struct_size: std::mem::size_of::<ffi::NativeWindow>() as u32,
                kind,
                display,
                window,
                visual_id,
            },
        }
    }

    fn as_raw(&self) -> &ffi::NativeWindow {
        &self.raw
    }
}

fn pointer_bits(pointer: NonNull<c_void>) -> u64 {
    u64::try_from(pointer.as_ptr() as usize)
        .expect("Fission's fixed-width native-window ABI requires pointers no wider than 64 bits")
}

/// Owner-thread Ganesh context backed by Vulkan native presentation.
#[derive(Clone)]
pub struct GaneshContext {
    inner: Rc<GaneshContextInner>,
}

struct GaneshContextInner {
    raw: ffi::ContextHandle,
    thread: ThreadAffinity,
    _engine: Engine,
}

impl GaneshContext {
    pub fn new_vulkan(engine: &Engine, compatible_window: NativeWindow) -> Result<Self> {
        let raw_engine = engine.raw_for_owner("GaneshContext::new_vulkan")?;
        let missing = REQUIRED_FEATURES & !engine.build_info().feature_bits;
        if missing != 0 {
            return Err(Error::local(
                ErrorKind::Unsupported,
                "GaneshContext::new_vulkan",
                format!("bridge does not advertise required Ganesh feature bits 0x{missing:016x}"),
            ));
        }
        let mut raw = 0;
        let mut error = ffi::Error::default();
        // SAFETY: the engine is live on its owner thread, the native descriptor
        // is initialized, and both outputs remain valid for the call.
        let status = unsafe {
            ffi::fission_skia_context_create_ganesh_vulkan(
                raw_engine,
                compatible_window.as_raw(),
                &mut raw,
                &mut error,
            )
        };
        status_result(status, &error)?;
        if raw == 0 {
            return Err(Error::local(
                ErrorKind::Internal,
                "GaneshContext::new_vulkan",
                "bridge returned a null context handle after reporting success",
            ));
        }
        Ok(Self {
            inner: Rc::new(GaneshContextInner {
                raw,
                thread: ThreadAffinity::current(),
                _engine: engine.clone(),
            }),
        })
    }

    pub fn trim_memory(&self, pressure: MemoryPressure) -> Result<()> {
        self.inner
            .thread
            .ensure_owner("GaneshContext::trim_memory")?;
        let mut error = ffi::Error::default();
        // SAFETY: the context is live and owner-thread access was checked.
        let status = unsafe {
            ffi::fission_skia_context_trim_memory(self.inner.raw, pressure.as_ffi(), &mut error)
        };
        status_result(status, &error)
    }
}

impl Drop for GaneshContextInner {
    fn drop(&mut self) {
        debug_assert!(
            self.thread.is_owner(),
            "Skia Ganesh context dropped off its owner thread"
        );
        let mut error = ffi::Error::default();
        // SAFETY: this is the sole native context owner. GaneshSurface retains
        // a clone of its context, so no safe child remains alive here.
        let status = unsafe { ffi::fission_skia_context_destroy(self.raw, &mut error) };
        debug_assert_eq!(
            status,
            ffi::STATUS_OK,
            "native Skia Ganesh context destruction failed"
        );
    }
}

/// Owner-thread Ganesh swapchain surface with explicit presentation ordering.
pub struct GaneshSurface {
    raw: ffi::SurfaceHandle,
    width: u32,
    height: u32,
    thread: ThreadAffinity,
    _context: GaneshContext,
}

impl GaneshSurface {
    pub fn new(
        context: &GaneshContext,
        window: NativeWindow,
        width: u32,
        height: u32,
    ) -> Result<Self> {
        context.inner.thread.ensure_owner("GaneshSurface::new")?;
        validate_extent(width, height, "GaneshSurface::new")?;
        let mut raw = 0;
        let mut error = ffi::Error::default();
        // SAFETY: the context and window are live on the owner thread and the
        // output handle remains valid for the duration of the call.
        let status = unsafe {
            ffi::fission_skia_surface_create_ganesh(
                context.inner.raw,
                window.as_raw(),
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
                "GaneshSurface::new",
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

    pub fn is_zero_sized(&self) -> bool {
        self.width == 0 || self.height == 0
    }

    pub fn resize(&mut self, window: NativeWindow, width: u32, height: u32) -> Result<()> {
        self.thread.ensure_owner("GaneshSurface::resize")?;
        validate_extent(width, height, "GaneshSurface::resize")?;
        let mut error = ffi::Error::default();
        // SAFETY: this wrapper uniquely owns the live surface, and the window
        // descriptor is initialized and valid for the bridge call.
        let status = unsafe {
            ffi::fission_skia_surface_resize_ganesh(
                self.raw,
                window.as_raw(),
                width,
                height,
                &mut error,
            )
        };
        status_result(status, &error)?;
        self.width = width;
        self.height = height;
        Ok(())
    }

    /// Renders one frame and moves the surface into ready-to-present state.
    pub fn execute_frame(&mut self, frame: &Frame) -> Result<()> {
        self.thread.ensure_owner("GaneshSurface::execute_frame")?;
        let encoded = frame.encode()?;
        let raw = encoded.as_raw();
        let mut error = ffi::Error::default();
        // SAFETY: encoded owns every referenced array and resource for the
        // call, while this wrapper provides unique owner-thread surface access.
        let status = unsafe { ffi::fission_skia_surface_execute_frame(self.raw, &raw, &mut error) };
        status_result(status, &error)
    }

    /// Presents the frame made ready by the preceding successful execution.
    pub fn present(&mut self) -> Result<()> {
        self.thread.ensure_owner("GaneshSurface::present")?;
        let mut error = ffi::Error::default();
        // SAFETY: this wrapper uniquely owns the live surface on its owner
        // thread; the bridge validates its presentation state.
        let status = unsafe { ffi::fission_skia_surface_present(self.raw, &mut error) };
        status_result(status, &error)
    }
}

impl Drop for GaneshSurface {
    fn drop(&mut self) {
        debug_assert!(
            self.thread.is_owner(),
            "Skia Ganesh surface dropped off its owner thread"
        );
        let mut error = ffi::Error::default();
        // SAFETY: GaneshSurface uniquely owns the native surface handle.
        let status = unsafe { ffi::fission_skia_surface_destroy(self.raw, &mut error) };
        debug_assert_eq!(
            status,
            ffi::STATUS_OK,
            "native Skia Ganesh surface destruction failed"
        );
    }
}

fn validate_extent(width: u32, height: u32, operation: &str) -> Result<()> {
    if width > i32::MAX as u32 || height > i32::MAX as u32 {
        Err(Error::local(
            ErrorKind::InvalidArgument,
            operation,
            "Ganesh surface dimensions exceed Skia's signed coordinate limit",
        ))
    } else {
        Ok(())
    }
}
