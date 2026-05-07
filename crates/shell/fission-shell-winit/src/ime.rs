use fission_core::env::ImeHandler;
use fission_render::LayoutRect;
use std::sync::{Arc, Mutex};
use winit::window::Window;

#[derive(Default)]
pub struct DesktopImeHandler {
    window: Mutex<Option<Arc<Window>>>,
}

impl DesktopImeHandler {
    pub fn set_window(&self, window: Option<Arc<Window>>) {
        *self.window.lock().expect("ime handler lock poisoned") = window;
    }
}

impl ImeHandler for DesktopImeHandler {
    fn set_ime_allowed(&self, allowed: bool) {
        if let Some(window) = self
            .window
            .lock()
            .expect("ime handler lock poisoned")
            .as_ref()
        {
            window.set_ime_allowed(allowed);
        }
    }
    fn set_ime_cursor_area(&self, rect: LayoutRect) {
        if let Some(window) = self
            .window
            .lock()
            .expect("ime handler lock poisoned")
            .as_ref()
        {
            // Position relative to window
            window.set_ime_cursor_area(
                winit::dpi::PhysicalPosition::new(rect.x() as f64, rect.y() as f64),
                winit::dpi::PhysicalSize::new(rect.width() as u32, rect.height() as u32),
            );
        }
    }
}
