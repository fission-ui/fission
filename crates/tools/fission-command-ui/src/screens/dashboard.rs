use super::title_block;
use crate::actions::{navigate, request_command, Navigate, RequestCommand};
use crate::commands::UiCommand;
use crate::components::{ActionButton, ButtonTone, DeviceTable, KeyValueRow};
use crate::routes::UiRoute;
use crate::state::{target_label, UiState};
use crate::theme::UiPalette;
use fission::prelude::*;

#[derive(Clone)]
pub struct DashboardScreen;

impl Widget<UiState> for DashboardScreen {
    fn build(&self, ctx: &mut BuildCtx<UiState>, view: &View<UiState>) -> Node {
        let palette = UiPalette::for_mode(view.state.theme_mode);
        let refresh = with_reducer!(ctx, RequestCommand(UiCommand::Refresh), request_command);
        let doctor = with_reducer!(ctx, Navigate(UiRoute::Doctor), navigate);
        let run = with_reducer!(ctx, Navigate(UiRoute::Run), navigate);
        let build = with_reducer!(ctx, Navigate(UiRoute::Build), navigate);
        let project = with_reducer!(ctx, Navigate(UiRoute::Project), navigate);
        let target_summary = if view.state.targets.is_empty() {
            "No configured targets".to_string()
        } else {
            view.state
                .targets
                .iter()
                .copied()
                .map(target_label)
                .collect::<Vec<_>>()
                .join(", ")
        };
        Column {
            gap: Some(1.0),
            children: vec![
                title_block(
                    "Dashboard",
                    "Manage this Fission app without memorising command syntax.",
                    palette.accent,
                    palette.muted,
                ),
                Row {
                    gap: Some(2.0),
                    children: vec![
                        KeyValueRow::new("Project", view.state.project_name.clone())
                            .build(ctx, view),
                        KeyValueRow::new("Theme", view.state.theme_mode.label()).build(ctx, view),
                    ],
                    ..Default::default()
                }
                .into_node(),
                KeyValueRow::new("Targets", target_summary).build(ctx, view),
                Row {
                    gap: Some(1.0),
                    children: vec![
                        ActionButton::new("Refresh", refresh)
                            .tone(ButtonTone::Neutral)
                            .build(ctx, view),
                        ActionButton::new("Check setup", doctor)
                            .tone(ButtonTone::Primary)
                            .build(ctx, view),
                        ActionButton::new("Run app", run)
                            .tone(ButtonTone::Success)
                            .build(ctx, view),
                        ActionButton::new("Build", build)
                            .tone(ButtonTone::Neutral)
                            .build(ctx, view),
                        ActionButton::new("Project setup", project)
                            .tone(ButtonTone::Neutral)
                            .build(ctx, view),
                    ],
                    ..Default::default()
                }
                .into_node(),
                Text::new("Available devices")
                    .color(palette.accent)
                    .into_node(),
                DeviceTable {
                    devices: view.state.devices.clone(),
                    selectable: false,
                    max_rows: 7,
                }
                .build(ctx, view),
            ],
            ..Default::default()
        }
        .into_node()
    }
}
