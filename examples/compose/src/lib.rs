mod actions;
mod components;
mod i18n;
mod pages;
mod router;
pub mod state;

pub use router::App;
pub use state::AppState;

use fission::prelude::*;

pub fn run_desktop() -> anyhow::Result<()> {
    let env = i18n::create_env()?;
    let title = env.tr("app.title");
    DesktopApp::<AppState, _>::new(App)
        .with_title(title)
        .with_env(env)
        .with_sync_env(|state, env| {
            env.locale = state.locale.clone();
            env.window.title = WindowTitle::plain(env.tr("app.title"));
        })
        .with_route_handler(actions::shell_route_changed)
        .run()
}

#[cfg(test)]
mod tests;
