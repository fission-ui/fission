mod app;
mod charts;
mod data;
mod doc_capture_view;
mod gallery_compact;
mod gallery_content;
mod gallery_controls;
mod gallery_expanded;
mod gallery_sidebar;
mod gallery_sidebar_button;
mod gallery_switch_control;
mod layout;
mod showcase;
mod showcase_chart_card;
mod showcase_grid;
mod showcase_metric_card;
mod state;
mod style;

pub use app::GalleryApp;
pub use state::GalleryState;

use fission::prelude::*;

pub fn configure_embedded_env(state: &GalleryState, env: &mut Env) {
    env.theme = if state.dark_theme {
        Theme::dark()
    } else {
        Theme::default()
    };
}

#[cfg(not(any(target_arch = "wasm32", target_os = "android", target_os = "ios")))]
pub fn run_desktop() -> anyhow::Result<()> {
    DesktopApp::<GalleryState, _>::new(GalleryApp)
        .with_title("Fission Chart Gallery")
        .with_sync_env(|state: &GalleryState, env: &mut Env| {
            configure_embedded_env(state, env);
        })
        .run()
}
