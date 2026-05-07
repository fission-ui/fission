use fission_core::env::ImeHandler;
use fission_render::LayoutRect;
use std::sync::Arc;
use winit::window::Window;

pub struct DesktopImeHandler {
    window: Arc<Window>,
}

impl DesktopImeHandler {
    pub fn new(window: Arc<Window>) -> Self {
        Self { window }
    }
}

impl ImeHandler for DesktopImeHandler {
    fn set_ime_allowed(&self, allowed: bool) {
        self.window.set_ime_allowed(allowed);
    }
    fn set_ime_cursor_area(&self, rect: LayoutRect) {
        // Position relative to window
        self.window.set_ime_cursor_area(
            winit::dpi::PhysicalPosition::new(rect.x() as f64, rect.y() as f64),
            winit::dpi::PhysicalSize::new(rect.width() as u32, rect.height() as u32),
        );
    }
}
