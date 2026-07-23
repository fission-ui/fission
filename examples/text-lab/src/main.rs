use anyhow::Result;
use fission::prelude::DesktopApp;

mod state;
mod text_lab_app;
mod text_lab_content;
mod text_lab_modal;

use state::TextLabState;
use text_lab_app::TextLabApp;

fn main() -> Result<()> {
    DesktopApp::<TextLabState, _>::new(TextLabApp).run()
}
