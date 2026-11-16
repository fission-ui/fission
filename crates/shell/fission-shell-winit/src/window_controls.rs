use winit::{event_loop::ActiveEventLoop, window::Window};

#[cfg(not(any(target_arch = "wasm32", target_os = "android", target_os = "ios")))]
mod desktop;
#[cfg(not(any(target_arch = "wasm32", target_os = "android", target_os = "ios")))]
pub(crate) use desktop::{WindowControls, WindowDragState};

#[cfg(any(target_arch = "wasm32", target_os = "android", target_os = "ios"))]
pub(crate) fn register_unsupported(registry: &mut fission_shell::async_host::AsyncRegistry) {
    registry.register_operation_capability(fission_core::WINDOW_CONTROL, |_, _| async {
        Err(fission_core::WindowControlError::Unsupported)
    });
}

#[cfg(feature = "tray")]
pub(crate) fn request_close<S: fission_core::GlobalState>(
    window: &Window,
    event_loop: &ActiveEventLoop,
    tray: Option<&crate::tray::ActiveTray<S>>,
) {
    if let Some(tray) =
        tray.filter(|tray| tray.close_behavior() == crate::tray::WindowCloseBehavior::HideToTray)
    {
        crate::tray::hide_window_to_tray(window, tray.app_switcher_policy());
    } else {
        event_loop.exit();
    }
}

#[cfg(not(feature = "tray"))]
pub(crate) fn request_close(_window: &Window, event_loop: &ActiveEventLoop) {
    event_loop.exit();
}
