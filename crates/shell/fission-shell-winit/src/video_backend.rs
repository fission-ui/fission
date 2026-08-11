#![allow(unexpected_cfgs)]

use fission_shell::{VideoBackend, VideoEvent, VideoPlayer};
use std::sync::Arc;
#[cfg(target_arch = "wasm32")]
use winit::platform::web::WindowExtWebSys;
use winit::window::Window;

#[cfg(target_os = "android")]
pub use android::AndroidVideoBackend;
#[cfg(target_os = "ios")]
pub use ios::IosVideoBackend;
#[cfg(all(target_os = "linux", feature = "video"))]
pub use linux::LinuxVideoBackend;
#[cfg(target_os = "macos")]
pub use mac::MacVideoBackend;
#[cfg(target_arch = "wasm32")]
pub use web::WebVideoBackend;
#[cfg(target_os = "windows")]
pub use windows::WindowsVideoBackend;

pub fn create_video_backend(window: Option<&Window>) -> Arc<dyn VideoBackend> {
    #[cfg(target_os = "macos")]
    if let Some(window) = window {
        if let Some(backend) = MacVideoBackend::try_new(window) {
            return Arc::new(backend);
        }
    }

    #[cfg(target_os = "ios")]
    if let Some(window) = window {
        if let Some(backend) = IosVideoBackend::try_new(window) {
            return Arc::new(backend);
        }
    }

    #[cfg(target_arch = "wasm32")]
    {
        let canvas = window.and_then(|window| window.canvas());
        return Arc::new(WebVideoBackend::new(canvas));
    }

    #[cfg(target_os = "windows")]
    if let Some(window) = window {
        if let Some(backend) = WindowsVideoBackend::try_new(window) {
            return Arc::new(backend);
        }
    }
    #[cfg(target_os = "windows")]
    panic!("Fission Video for Windows requires a Win32 window handle");

    #[cfg(all(target_os = "linux", feature = "video"))]
    if let Some(window) = window {
        if let Some(backend) = LinuxVideoBackend::try_new(window) {
            return Arc::new(backend);
        }
    }
    #[cfg(all(target_os = "linux", feature = "video"))]
    panic!("Fission Video for Linux requires an X11 or Wayland window handle");

    #[cfg(all(target_os = "linux", not(feature = "video")))]
    {
        let _ = window;
        return Arc::new(unsupported::UnsupportedVideoBackend::new(
            "Fission native Video on Linux requires the `video` feature and GStreamer development packages. Enable `fission = { version = \"...\", features = [\"desktop\", \"video\"] }`; on Debian/Ubuntu install `sudo apt install libglib2.0-dev libgstreamer1.0-dev libgstreamer-plugins-base1.0-dev`. Static site, SSR, and Web video do not require this native backend.",
        ));
    }

    #[cfg(target_os = "android")]
    {
        let _ = window;
        return Arc::new(AndroidVideoBackend::new());
    }

    #[cfg(any(target_os = "macos", target_os = "ios"))]
    panic!(
        "Fission Video requires a native window on this target; no state-only mock fallback exists"
    );

    #[cfg(not(any(
        target_os = "macos",
        target_os = "ios",
        target_os = "windows",
        all(target_os = "linux", feature = "video"),
        target_os = "android",
        target_arch = "wasm32"
    )))]
    panic!("Fission Video is unsupported on this platform");
}

#[cfg(any(target_arch = "wasm32", test))]
pub(super) fn web_video_document_position(
    canvas_viewport_left: f64,
    canvas_viewport_top: f64,
    document_scroll_x: f64,
    document_scroll_y: f64,
    frame_x: f32,
    frame_y: f32,
) -> (f64, f64) {
    (
        canvas_viewport_left + document_scroll_x + f64::from(frame_x),
        canvas_viewport_top + document_scroll_y + f64::from(frame_y),
    )
}

#[cfg(target_os = "android")]
mod android;
#[cfg(target_os = "ios")]
#[allow(unexpected_cfgs)]
mod ios;
#[cfg(all(target_os = "linux", feature = "video"))]
mod linux;
#[cfg(target_os = "macos")]
#[allow(unexpected_cfgs)]
mod mac;
#[cfg(test)]
mod tests;
#[cfg(all(target_os = "linux", not(feature = "video")))]
mod unsupported;
#[cfg(target_arch = "wasm32")]
mod web;
#[cfg(target_os = "windows")]
mod windows;

#[cfg(test)]
mod web_position_tests {
    use super::web_video_document_position;

    #[test]
    fn document_position_combines_canvas_scroll_and_frame_origins() {
        let position = web_video_document_position(120.5, -350.0, 80.0, 500.0, 12.25, 42.0);

        assert_eq!(position, (212.75, 192.0));
    }
}
