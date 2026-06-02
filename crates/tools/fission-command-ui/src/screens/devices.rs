use super::title_block;
use crate::actions::{request_command, RequestCommand};
use crate::commands::UiCommand;
use crate::components::{ActionButton, ButtonTone, DeviceTable, KeyValueRow};
use crate::state::UiState;
use crate::theme::UiPalette;
use fission::prelude::*;

#[derive(Clone)]
pub struct DevicesScreen;

impl From<DevicesScreen> for Widget {
    fn from(_component: DevicesScreen) -> Self {
        let (ctx, view) = fission::build::current::<UiState>();
        let palette = UiPalette::for_mode(view.state().theme_mode);
        let refresh = with_reducer!(ctx, RequestCommand(UiCommand::Refresh), request_command);
        Column {
            gap: Some(1.0),
            children: vec![
                title_block(
                    "Devices",
                    "Select the browser, desktop host, simulator, emulator, or attached device used by run and logs.",
                    palette.accent,
                    palette.muted,
                ),
                Row {
                    gap: Some(2.0),
                    children: vec![
                        KeyValueRow::new("Selected target", view.state().selected_target_label())
                            .into(),
                        KeyValueRow::new("Selected device", view.state().selected_device_label())
                            .into(),
                        ActionButton::new("Refresh devices", refresh)
                            .tone(ButtonTone::Primary)
                            .into(),
                    ],
                    ..Default::default()
                }
                .into(),
                DeviceTable {
                    devices: view.state().devices.clone(),
                    selectable: true,
                    max_rows: 12,
                }
                .into(),
            ],
            ..Default::default()
        }
        .into()
    }
}
