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

use fission::prelude::*;
use gallery_app::GalleryApp;
pub(crate) use state::GalleryState;

fn main() -> anyhow::Result<()> {
    DesktopApp::<GalleryState, _>::new(GalleryApp).run()
}
