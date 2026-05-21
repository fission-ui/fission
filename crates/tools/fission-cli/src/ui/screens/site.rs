use super::title_block;
use crate::ui::actions::{
    request_command, set_host, set_port, toggle_no_open, toggle_release, RequestCommand, SetHost,
    SetPort, ToggleNoOpen, ToggleRelease,
};
use crate::ui::commands::UiCommand;
use crate::ui::components::{ActionButton, ButtonTone, FormTextField, KeyValueRow, TogglePill};
use crate::ui::state::UiState;
use crate::ui::theme::UiPalette;
use fission::prelude::*;

#[derive(Clone)]
pub(crate) struct SiteScreen;

impl Widget<UiState> for SiteScreen {
    fn build(&self, ctx: &mut BuildCtx<UiState>, view: &View<UiState>) -> Node {
        let palette = UiPalette::for_mode(view.state.theme_mode);
        let build = with_reducer!(ctx, RequestCommand(UiCommand::SiteBuild), request_command);
        let check = with_reducer!(ctx, RequestCommand(UiCommand::SiteCheck), request_command);
        let routes = with_reducer!(ctx, RequestCommand(UiCommand::SiteRoutes), request_command);
        let serve = with_reducer!(ctx, RequestCommand(UiCommand::SiteServe), request_command);
        let release = with_reducer!(ctx, ToggleRelease, toggle_release);
        let no_open = with_reducer!(ctx, ToggleNoOpen, toggle_no_open);
        let host = with_reducer!(ctx, SetHost(String::new()), set_host);
        let port = with_reducer!(ctx, SetPort(String::new()), set_port);

        Column {
            gap: Some(1.0),
            children: vec![
                title_block(
                    "Static site",
                    "Build, validate, serve, and inspect routes for projects with the site target enabled.",
                    palette.accent,
                    palette.muted,
                ),
                KeyValueRow::new("Project", view.state.project_dir.display().to_string())
                    .build(ctx, view),
                Row {
                    gap: Some(1.0),
                    children: vec![
                        FormTextField::new(
                            "cli_ui_site_host",
                            "Host",
                            view.state.host.clone(),
                            "127.0.0.1",
                            host,
                        )
                        .width(24.0)
                        .build(ctx, view),
                        FormTextField::new("cli_ui_site_port", "Port", view.state.port.clone(), "8123", port)
                            .width(12.0)
                            .build(ctx, view),
                    ],
                    ..Default::default()
                }
                .into_node(),
                Row {
                    gap: Some(1.0),
                    children: vec![
                        TogglePill::new("Release", view.state.release, release).build(ctx, view),
                        TogglePill::new("No open", view.state.no_open, no_open).build(ctx, view),
                    ],
                    ..Default::default()
                }
                .into_node(),
                Row {
                    gap: Some(1.0),
                    children: vec![
                        ActionButton::new("Build site", build)
                            .tone(ButtonTone::Primary)
                            .width(18.0)
                            .build(ctx, view),
                        ActionButton::new("Check site", check)
                            .tone(ButtonTone::Success)
                            .width(18.0)
                            .build(ctx, view),
                        ActionButton::new("List routes", routes)
                            .tone(ButtonTone::Neutral)
                            .width(18.0)
                            .build(ctx, view),
                        ActionButton::new("Serve site", serve)
                            .tone(ButtonTone::Warning)
                            .width(18.0)
                            .build(ctx, view),
                    ],
                    ..Default::default()
                }
                .into_node(),
                Text::new("Serve runs in the background and writes output under .fission/ui.")
                    .color(palette.muted)
                    .into_node(),
            ],
            ..Default::default()
        }
        .into_node()
    }
}
