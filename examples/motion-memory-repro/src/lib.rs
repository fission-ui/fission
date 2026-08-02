mod app;
mod fixtures;
mod layout;
mod palette;
mod repro_row;
mod repro_screen;
mod scenario;

pub use app::MotionMemoryReproApp;

#[cfg(not(any(target_arch = "wasm32", target_os = "android", target_os = "ios")))]
pub fn run_desktop() -> anyhow::Result<()> {
    fission::prelude::DesktopApp::<(), _>::new(MotionMemoryReproApp::from_env()?).run()
}
