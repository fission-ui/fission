mod app;
mod fixtures;
mod layout;
mod palette;
mod repro_row;
mod repro_screen;
mod scenario;

use app::MotionMemoryReproApp;
use fission::prelude::*;

fn main() -> anyhow::Result<()> {
    DesktopApp::<(), _>::new(MotionMemoryReproApp::from_env()?).run()
}
