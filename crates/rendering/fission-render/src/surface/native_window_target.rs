use std::any::Any;
use std::fmt;

use raw_window_handle::{RawDisplayHandle, RawWindowHandle};

use super::{SurfaceDescriptor, SurfaceKind, SurfaceTarget};

/// A native presentation target whose window-system objects remain host-owned.
///
/// This type copies opaque raw handles; it does not own or extend the lifetime
/// of the display connection or window. A platform host must retain that
/// ownership while a graphics session is attached to this target.
#[derive(Debug)]
pub struct NativeWindowTarget {
    descriptor: SurfaceDescriptor,
    display_handle: RawDisplayHandle,
    window_handle: RawWindowHandle,
}

impl NativeWindowTarget {
    /// Copies a host's native display and window handles into a surface target.
    ///
    /// # Safety
    ///
    /// Whenever the returned target is attached to a graphics session,
    /// `display_handle` and `window_handle` must identify the same live native
    /// window represented by `descriptor`. Every pointer and non-zero value
    /// consumed by that session must satisfy `raw-window-handle`'s validity
    /// contract. The caller must keep the underlying display connection,
    /// window, and referenced objects alive and unchanged until every graphics
    /// session attached to this target has detached.
    pub unsafe fn from_raw_handles(
        descriptor: SurfaceDescriptor,
        display_handle: RawDisplayHandle,
        window_handle: RawWindowHandle,
    ) -> Result<Self, NativeWindowTargetError> {
        if descriptor.kind != SurfaceKind::NativeWindow {
            return Err(NativeWindowTargetError::InvalidSurfaceKind(descriptor.kind));
        }

        Ok(Self {
            descriptor,
            display_handle,
            window_handle,
        })
    }

    pub const fn raw_display_handle(&self) -> RawDisplayHandle {
        self.display_handle
    }

    pub const fn raw_window_handle(&self) -> RawWindowHandle {
        self.window_handle
    }
}

impl SurfaceTarget for NativeWindowTarget {
    fn descriptor(&self) -> &SurfaceDescriptor {
        &self.descriptor
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeWindowTargetError {
    InvalidSurfaceKind(SurfaceKind),
}

impl fmt::Display for NativeWindowTargetError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidSurfaceKind(kind) => write!(
                formatter,
                "native window target requires SurfaceKind::NativeWindow, got {kind:?}"
            ),
        }
    }
}

impl std::error::Error for NativeWindowTargetError {}

#[cfg(test)]
mod tests {
    use raw_window_handle::{
        RawDisplayHandle, RawWindowHandle, XlibDisplayHandle, XlibWindowHandle,
    };

    use crate::capabilities::ColorFormat;

    use super::*;
    use crate::surface::{PhysicalSize, ScaleFactor, SurfaceId, ThreadAffinity};

    fn descriptor(kind: SurfaceKind) -> SurfaceDescriptor {
        SurfaceDescriptor {
            id: SurfaceId(41),
            kind,
            size: PhysicalSize::new(1280, 720),
            scale_factor: ScaleFactor::ONE,
            color_format: ColorFormat::Bgra8Srgb,
            thread_affinity: ThreadAffinity::CreatingThread,
        }
    }

    #[test]
    fn preserves_descriptor_and_copied_raw_handles() {
        let display = RawDisplayHandle::Xlib(XlibDisplayHandle::new(None, 2));
        let window = RawWindowHandle::Xlib(XlibWindowHandle::new(73));
        let expected_descriptor = descriptor(SurfaceKind::NativeWindow);

        // SAFETY: This contract test never dereferences or presents through the
        // inert handles, and the values remain unchanged for the target's life.
        let target = unsafe {
            NativeWindowTarget::from_raw_handles(expected_descriptor.clone(), display, window)
        }
        .unwrap();

        assert_eq!(target.descriptor(), &expected_descriptor);
        assert_eq!(target.raw_display_handle(), display);
        assert_eq!(target.raw_window_handle(), window);
        assert!(target
            .as_any()
            .downcast_ref::<NativeWindowTarget>()
            .is_some());
    }

    #[test]
    fn rejects_a_non_native_surface_descriptor() {
        let display = RawDisplayHandle::Xlib(XlibDisplayHandle::new(None, 0));
        let window = RawWindowHandle::Xlib(XlibWindowHandle::new(1));

        // SAFETY: The constructor rejects the descriptor before a backend can
        // consume the inert handles.
        let error = unsafe {
            NativeWindowTarget::from_raw_handles(descriptor(SurfaceKind::Headless), display, window)
        }
        .unwrap_err();

        assert_eq!(
            error,
            NativeWindowTargetError::InvalidSurfaceKind(SurfaceKind::Headless)
        );
    }
}
