mod actions;
mod app;
mod commands;
mod components;
mod density;
mod publish;
mod release_config;
mod routes;
mod screens;
mod state;
mod theme;

use anyhow::Result;
use std::path::PathBuf;

#[cfg(test)]
use theme::UiThemeMode;

pub use app::CliUiApp;
pub use publish::{
    default_publish_options, run_publish_tui, run_publish_window, PublishApp, PublishUiOptions,
    PublishUiState,
};
pub use release_config::{
    run_release_config_tui, ReleaseConfigEditorApp, ReleaseConfigEditorOptions,
    ReleaseConfigEditorState,
};
pub use state::UiState;

#[derive(Clone, Debug)]
pub struct UiOptions {
    pub project_dir: PathBuf,
    pub screenshot: Option<PathBuf>,
    pub exit_after_render: bool,
    pub width: Option<u16>,
    pub height: Option<u16>,
}

pub fn run_ui(options: UiOptions) -> Result<()> {
    let mut publish_options = publish::default_publish_options(options.project_dir);
    publish_options.screenshot = options.screenshot;
    publish_options.exit_after_render = options.exit_after_render;
    publish_options.width = options.width;
    publish_options.height = options.height;
    run_publish_tui(publish_options)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::routes::UiRoute;
    use crate::state::all_targets;
    use fission_command_core::Target;
    use std::path::PathBuf;

    #[test]
    fn cli_ui_renders_every_route_in_terminal_shell() {
        for route in UiRoute::ALL {
            let mut state = UiState {
                project_dir: PathBuf::from("."),
                project_name: "test-app".to_string(),
                app_id: "com.example.test".to_string(),
                project_status: "Project loaded".to_string(),
                targets: all_targets().to_vec(),
                selected_target: Some(Target::Web),
                route,
                host: "127.0.0.1".to_string(),
                port: "8123".to_string(),
                theme_mode: UiThemeMode::Dark,
                ..Default::default()
            };
            state.devices = vec![crate::state::UiDevice {
                id: "chrome".to_string(),
                name: "Chrome/Chromium".to_string(),
                target: Target::Web,
                kind: "browser".to_string(),
                status: "available".to_string(),
                detail: String::new(),
                available: true,
            }];

            let mut app = fission::terminal::TerminalApp::with_state(CliUiApp, state)
                .with_sync_env(|state, env| {
                    env.theme = match state.theme_mode {
                        UiThemeMode::Dark => fission::theme::Theme::dark(),
                        UiThemeMode::Light => fission::theme::Theme::default(),
                    };
                });
            app.render_frame(120, 40).expect("route should render");
        }
    }

    #[test]
    fn cli_ui_renders_confirmation_dialog() {
        let mut state = UiState {
            project_dir: PathBuf::from("."),
            project_name: "test-app".to_string(),
            app_id: "com.example.test".to_string(),
            project_status: "Project loaded".to_string(),
            targets: all_targets().to_vec(),
            selected_target: Some(Target::Web),
            host: "127.0.0.1".to_string(),
            port: "8123".to_string(),
            theme_mode: UiThemeMode::Dark,
            ..Default::default()
        };
        state.request_command_confirmation(crate::commands::UiCommand::RunSelected);

        let mut app = fission::terminal::TerminalApp::with_state(CliUiApp, state).with_sync_env(
            |state, env| {
                env.theme = match state.theme_mode {
                    UiThemeMode::Dark => fission::theme::Theme::dark(),
                    UiThemeMode::Light => fission::theme::Theme::default(),
                };
            },
        );
        let frame = app.render_frame(120, 40).expect("dialog should render");
        assert!(frame
            .as_plain_text()
            .contains("Confirm: run the selected target"));
        assert!(frame.as_plain_text().contains("Cancel"));
    }
}
