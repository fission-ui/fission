use std::sync::Arc;

use anyhow::{anyhow, Context};
use fission_render::surface::{NativeWindowTarget, SurfaceDescriptor, ThreadAffinity};
use raw_window_handle::{HasDisplayHandle, HasWindowHandle, RawDisplayHandle, RawWindowHandle};
use winit::window::Window;

/// Couples copied native handles to the Winit window that keeps them valid.
///
/// The native presenter retains this holder until its graphics session has
/// detached. Exposing only a borrowed target keeps that lifetime rule local to
/// the presenter boundary instead of distributing raw handles through the
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
        validate_native_handle_pair(display_handle, window_handle)?;
        let required_affinity = native_thread_affinity();
        if descriptor.thread_affinity != required_affinity {
            return Err(anyhow!(
                "native Skia presentation on this platform requires {required_affinity:?} surface affinity, got {:?}",
                descriptor.thread_affinity
            ));
        }

        // SAFETY: `window` owns the display/window resources and is retained in
        // this holder. The presenter retains this holder alongside its attached
        // graphics session, as documented above.
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

#[cfg(target_os = "linux")]
fn validate_native_handle_pair(
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

#[cfg(target_os = "macos")]
fn validate_native_handle_pair(
    display_handle: RawDisplayHandle,
    window_handle: RawWindowHandle,
) -> anyhow::Result<()> {
    match (display_handle, window_handle) {
        (RawDisplayHandle::AppKit(_), RawWindowHandle::AppKit(_)) => Ok(()),
        _ => Err(anyhow!(
            "native Skia presentation on macOS requires a matching AppKit display/window handle pair"
        )),
    }
}

#[cfg(target_os = "ios")]
fn validate_native_handle_pair(
    display_handle: RawDisplayHandle,
    window_handle: RawWindowHandle,
) -> anyhow::Result<()> {
    match (display_handle, window_handle) {
        (RawDisplayHandle::UiKit(_), RawWindowHandle::UiKit(_)) => Ok(()),
        _ => Err(anyhow!(
            "native Skia presentation on iOS requires a matching UIKit display/window handle pair"
        )),
    }
}

#[cfg(target_os = "windows")]
fn validate_native_handle_pair(
    display_handle: RawDisplayHandle,
    window_handle: RawWindowHandle,
) -> anyhow::Result<()> {
    match (display_handle, window_handle) {
        (RawDisplayHandle::Windows(_), RawWindowHandle::Win32(_)) => Ok(()),
        _ => Err(anyhow!(
            "native Skia presentation on Windows requires a matching Windows/Win32 display and window handle pair"
        )),
    }
}

#[cfg(target_os = "android")]
fn validate_native_handle_pair(
    display_handle: RawDisplayHandle,
    window_handle: RawWindowHandle,
) -> anyhow::Result<()> {
    match (display_handle, window_handle) {
        (RawDisplayHandle::Android(_), RawWindowHandle::AndroidNdk(_)) => Ok(()),
        _ => Err(anyhow!(
            "native Skia presentation on Android requires a matching Android/AndroidNdk display and window handle pair"
        )),
    }
}

pub(super) fn native_thread_affinity() -> ThreadAffinity {
    if cfg!(any(target_os = "macos", target_os = "ios")) {
        ThreadAffinity::MainThread
    } else {
        ThreadAffinity::CreatingThread
    }
}

#[cfg(test)]
mod tests {
    use std::ptr::NonNull;

    #[cfg(target_os = "linux")]
    use std::num::NonZeroU32;

    #[cfg(target_os = "android")]
    use raw_window_handle::{AndroidDisplayHandle, AndroidNdkWindowHandle};
    #[cfg(target_os = "macos")]
    use raw_window_handle::{AppKitDisplayHandle, AppKitWindowHandle};
    #[cfg(target_os = "ios")]
    use raw_window_handle::{UiKitDisplayHandle, UiKitWindowHandle};
    #[cfg(target_os = "linux")]
    use raw_window_handle::{
        WaylandDisplayHandle, WaylandWindowHandle, WebDisplayHandle, WebWindowHandle,
        XcbDisplayHandle, XcbWindowHandle, XlibDisplayHandle, XlibWindowHandle,
    };
    #[cfg(target_os = "windows")]
    use raw_window_handle::{Win32WindowHandle, WindowsDisplayHandle};

    use super::*;

    #[cfg(target_os = "linux")]
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
            validate_native_handle_pair(display, window).unwrap();
        }
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn rejects_unsupported_or_mismatched_handle_pairs_clearly() {
        let unsupported = validate_native_handle_pair(
            RawDisplayHandle::Web(WebDisplayHandle::new()),
            RawWindowHandle::Web(WebWindowHandle::new(1)),
        )
        .unwrap_err();
        assert!(unsupported
            .to_string()
            .contains("matching Xlib, Xcb, or Wayland"));

        let mismatched = validate_native_handle_pair(
            RawDisplayHandle::Xlib(XlibDisplayHandle::new(None, 0)),
            RawWindowHandle::Xcb(XcbWindowHandle::new(NonZeroU32::new(1).unwrap())),
        );
        assert!(mismatched.is_err());
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn accepts_only_appkit_handle_pairs_on_macos() {
        let pointer = NonNull::dangling();
        validate_native_handle_pair(
            RawDisplayHandle::AppKit(AppKitDisplayHandle::new()),
            RawWindowHandle::AppKit(AppKitWindowHandle::new(pointer)),
        )
        .unwrap();
        assert!(validate_native_handle_pair(
            RawDisplayHandle::AppKit(AppKitDisplayHandle::new()),
            RawWindowHandle::UiKit(raw_window_handle::UiKitWindowHandle::new(pointer)),
        )
        .is_err());
    }

    #[cfg(target_os = "ios")]
    #[test]
    fn accepts_only_uikit_handle_pairs_on_ios() {
        let pointer = NonNull::dangling();
        validate_native_handle_pair(
            RawDisplayHandle::UiKit(UiKitDisplayHandle::new()),
            RawWindowHandle::UiKit(UiKitWindowHandle::new(pointer)),
        )
        .unwrap();
        assert!(validate_native_handle_pair(
            RawDisplayHandle::UiKit(UiKitDisplayHandle::new()),
            RawWindowHandle::AppKit(raw_window_handle::AppKitWindowHandle::new(pointer)),
        )
        .is_err());
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn accepts_only_win32_handle_pairs_on_windows() {
        let hwnd = std::num::NonZeroIsize::new(1).unwrap();
        validate_native_handle_pair(
            RawDisplayHandle::Windows(WindowsDisplayHandle::new()),
            RawWindowHandle::Win32(Win32WindowHandle::new(hwnd)),
        )
        .unwrap();
        assert!(validate_native_handle_pair(
            RawDisplayHandle::Windows(WindowsDisplayHandle::new()),
            RawWindowHandle::AppKit(raw_window_handle::AppKitWindowHandle::new(
                NonNull::dangling(),
            )),
        )
        .is_err());
    }

    #[cfg(target_os = "android")]
    #[test]
    fn accepts_only_android_ndk_handle_pairs_on_android() {
        let pointer = NonNull::dangling();
        validate_native_handle_pair(
            RawDisplayHandle::Android(AndroidDisplayHandle::new()),
            RawWindowHandle::AndroidNdk(AndroidNdkWindowHandle::new(pointer)),
        )
        .unwrap();
        assert!(validate_native_handle_pair(
            RawDisplayHandle::Android(AndroidDisplayHandle::new()),
            RawWindowHandle::AppKit(raw_window_handle::AppKitWindowHandle::new(pointer)),
        )
        .is_err());
    }

    #[test]
    fn native_surface_affinity_matches_platform_contract() {
        if cfg!(any(target_os = "macos", target_os = "ios")) {
            assert_eq!(native_thread_affinity(), ThreadAffinity::MainThread);
        } else {
            assert_eq!(native_thread_affinity(), ThreadAffinity::CreatingThread);
        }
    }
}
