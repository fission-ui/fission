mod colour_picker_section;
mod data_section;
mod display_section;
mod drag_drop;
mod feedback_section;
mod gallery_app;
mod gallery_header;
mod gallery_section;
mod input_section;
mod navigation_section;
mod overlay_section;
mod state;

pub use gallery_app::GalleryApp;
pub use state::GalleryState;

#[cfg(not(any(target_arch = "wasm32", target_os = "android", target_os = "ios")))]
pub fn run_desktop() -> anyhow::Result<()> {
    fission::prelude::DesktopApp::<GalleryState, _>::new(GalleryApp).run()
}
