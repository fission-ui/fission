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

use app::GalleryApp;
use fission::prelude::DesktopApp;
use state::GalleryState;

fn main() -> anyhow::Result<()> {
    let app = DesktopApp::<GalleryState, _>::new(GalleryApp)
        .with_title("Fission Chart Gallery")
        .with_sync_env(|state: &GalleryState, env: &mut fission::core::Env| {
            env.theme = if state.dark_theme {
                fission::theme::Theme::dark()
            } else {
                fission::theme::Theme::default()
            };
        });

    app.run()
}
