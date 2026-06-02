mod app;
mod charts;
mod data;
mod showcase;
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
