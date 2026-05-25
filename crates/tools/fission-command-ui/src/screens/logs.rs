use super::title_block;
use crate::actions::{request_command, RequestCommand};
use crate::commands::UiCommand;
use crate::components::{ActionButton, ButtonTone, DeviceTable, KeyValueRow, TargetPicker};
use crate::state::{UiDevice, UiState};
use crate::theme::UiPalette;
use fission::prelude::*;

#[derive(Clone)]
pub struct LogsScreen;

impl Widget<UiState> for LogsScreen {
    fn build(&self, ctx: &mut BuildCtx<UiState>, view: &View<UiState>) -> Node {
        let palette = UiPalette::for_mode(view.state.theme_mode);
        let snapshot = with_reducer!(
            ctx,
            RequestCommand(UiCommand::LogsSnapshot),
            request_command
        );
        let follow = with_reducer!(ctx, RequestCommand(UiCommand::LogsFollow), request_command);
        Column {
            gap: Some(1.0),
            children: vec![
                title_block(
                    "Logs",
                    "Read the current log buffer or start a background log follower for the selected target and device.",
                    palette.accent,
                    palette.muted,
                ),
                Row {
                    gap: Some(2.0),
                    children: vec![
                        KeyValueRow::new("Target", view.state.selected_target_label()).build(ctx, view),
                        KeyValueRow::new("Device", view.state.selected_device_label()).build(ctx, view),
                    ],
                    ..Default::default()
                }
                .into_node(),
                TargetPicker {
                    configured_only: true,
                }
                .build(ctx, view),
                DeviceTable {
                    devices: current_target_devices(view),
                    selectable: true,
                    max_rows: 7,
                }
                .build(ctx, view),
                Row {
                    gap: Some(1.0),
                    children: vec![
                        ActionButton::new("Read logs", snapshot)
                            .tone(ButtonTone::Primary)
                            .width(18.0)
                            .build(ctx, view),
                        ActionButton::new("Follow logs", follow)
                            .tone(ButtonTone::Warning)
                            .width(18.0)
                            .build(ctx, view),
                    ],
                    ..Default::default()
                }
                .into_node(),
            ],
            ..Default::default()
        }
        .into_node()
    }
}

fn current_target_devices(view: &View<UiState>) -> Vec<UiDevice> {
    view.state
        .target_devices()
        .into_iter()
        .cloned()
        .collect::<Vec<_>>()
}
