mod state;
mod text_lab_app;
mod text_lab_content;
mod text_lab_modal;

pub use state::TextLabState;
pub use text_lab_app::TextLabApp;

#[cfg(not(any(target_arch = "wasm32", target_os = "android", target_os = "ios")))]
pub fn run_desktop() -> anyhow::Result<()> {
    fission::prelude::DesktopApp::<TextLabState, _>::new(TextLabApp).run()
}
