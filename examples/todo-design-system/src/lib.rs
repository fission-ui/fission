//! A todo list styled entirely by a design system generated at build time.
//!
//! `build.rs` compiles a Design System Package into `TodoDesignSystem`. The
//! widgets read colours, spacing, type and component styles from the theme that
//! type produces, and every label from the translation files in `i18n/`.

use fission::prelude::*;

include!(concat!(env!("OUT_DIR"), "/todo_design_system.rs"));

mod state;
mod translations;
mod widgets;

pub use state::TodoState;
pub use translations::translation_bundles;
pub use widgets::TodoApp;

/// When embedded, the todo app keeps its own design system, which is the point of
/// the example, but follows the host's light or dark mode.
pub fn configure_embedded_env(_state: &TodoState, env: &mut Env) {
    env.theme = TodoDesignSystem::theme(env.theme.design_system.mode);
}

/// The todo list as another app mounts it. The host picks light or dark, so the
/// app hides its own theme switch.
pub fn embedded_state() -> TodoState {
    let mut state = TodoState::default();
    state.embedded = true;
    state
}

#[cfg(not(any(target_arch = "wasm32", target_os = "android", target_os = "ios")))]
pub fn run_desktop() -> anyhow::Result<()> {
    let mut env = Env::default();
    for bundle in translation_bundles() {
        env.i18n.add_bundle(bundle);
    }
    DesktopApp::<TodoState, _>::new(TodoApp)
        .with_env(env)
        .with_design_system::<TodoDesignSystem>(DesignMode::Light)
        .with_sync_env(|state: &TodoState, env: &mut Env| {
            env.theme = TodoDesignSystem::theme(state.theme_mode);
            let title = env.tr("todo.window_title");
            env.window.title = WindowTitle::plain(title);
        })
        .run()
}
