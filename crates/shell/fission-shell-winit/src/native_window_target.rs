use std::sync::Arc;

use anyhow::{anyhow, Context};
use fission_render::surface::{NativeWindowTarget, SurfaceDescriptor};
use raw_window_handle::{HasDisplayHandle, HasWindowHandle, RawDisplayHandle, RawWindowHandle};
use winit::window::Window;

/// Couples copied native handles to the Winit window that keeps them valid.
///
/// A future native presenter must retain this holder until its graphics session
/// has detached. Exposing only a borrowed target keeps that lifetime rule local
/// to the presenter boundary instead of distributing raw handles through the
/// shell.
#[allow(dead_code)]
pub(super) struct WinitNativeWindowTarget {
    target: NativeWindowTarget,
    _window: Arc<Window>,
}

#[allow(dead_code)]
impl WinitNativeWindowTarget {
    pub(super) fn new(window: Arc<Window>, descriptor: SurfaceDescriptor) -> anyhow::Result<Self> {
        let display_handle = window
            .display_handle()
            .context("Winit did not provide a native display handle")?
            .as_raw();
        let window_handle = window
            .window_handle()
            .context("Winit did not provide a native window handle")?
            .as_raw();
        validate_linux_handle_pair(display_handle, window_handle)?;

        // SAFETY: `window` owns the display/window resources and is retained in
        // this holder. The future presenter is required to retain this holder
        // alongside its attached graphics session, as documented above.
        let target = unsafe {
            NativeWindowTarget::from_raw_handles(descriptor, display_handle, window_handle)
        }?;

        Ok(Self {
            target,
            _window: window,
        })
    }

    pub(super) fn target(&self) -> &NativeWindowTarget {
        &self.target
    }
}

fn validate_linux_handle_pair(
    display_handle: RawDisplayHandle,
    window_handle: RawWindowHandle,
) -> anyhow::Result<()> {
    match (display_handle, window_handle) {
        (RawDisplayHandle::Xlib(_), RawWindowHandle::Xlib(_))
        | (RawDisplayHandle::Xcb(_), RawWindowHandle::Xcb(_))
        | (RawDisplayHandle::Wayland(_), RawWindowHandle::Wayland(_)) => Ok(()),
        _ => Err(anyhow!(
            "native Skia presentation on Linux requires a matching Xlib, Xcb, or Wayland display/window handle pair"
        )),
    }
}

#[cfg(test)]
mod tests {
    use std::num::NonZeroU32;
    use std::ptr::NonNull;

    use raw_window_handle::{
        WaylandDisplayHandle, WaylandWindowHandle, WebDisplayHandle, WebWindowHandle,
        XcbDisplayHandle, XcbWindowHandle, XlibDisplayHandle, XlibWindowHandle,
    };

    use super::*;

    #[test]
    fn accepts_supported_linux_handle_pairs() {
        let pointer = NonNull::dangling();
        let supported = [
            (
                RawDisplayHandle::Xlib(XlibDisplayHandle::new(None, 0)),
                RawWindowHandle::Xlib(XlibWindowHandle::new(1)),
            ),
            (
                RawDisplayHandle::Xcb(XcbDisplayHandle::new(None, 0)),
                RawWindowHandle::Xcb(XcbWindowHandle::new(NonZeroU32::new(1).unwrap())),
            ),
            (
                RawDisplayHandle::Wayland(WaylandDisplayHandle::new(pointer)),
                RawWindowHandle::Wayland(WaylandWindowHandle::new(pointer)),
            ),
        ];

        for (display, window) in supported {
            validate_linux_handle_pair(display, window).unwrap();
        }
    }

    #[test]
    fn rejects_unsupported_or_mismatched_handle_pairs_clearly() {
        let unsupported = validate_linux_handle_pair(
            RawDisplayHandle::Web(WebDisplayHandle::new()),
            RawWindowHandle::Web(WebWindowHandle::new(1)),
        )
        .unwrap_err();
        assert!(unsupported
            .to_string()
            .contains("matching Xlib, Xcb, or Wayland"));

        let mismatched = validate_linux_handle_pair(
            RawDisplayHandle::Xlib(XlibDisplayHandle::new(None, 0)),
            RawWindowHandle::Xcb(XcbWindowHandle::new(NonZeroU32::new(1).unwrap())),
        );
        assert!(mismatched.is_err());
    }
}
