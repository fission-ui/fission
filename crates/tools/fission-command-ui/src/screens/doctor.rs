use super::title_block;
use crate::actions::{request_command, toggle_strict, RequestCommand, ToggleStrict};
use crate::commands::UiCommand;
use crate::components::{ActionButton, ButtonTone, KeyValueRow, TargetPicker, TogglePill};
use crate::state::{target_label, UiState};
use crate::theme::UiPalette;
use fission::prelude::*;

#[derive(Clone)]
pub struct DoctorScreen;

impl From<DoctorScreen> for Widget {
    fn from(_component: DoctorScreen) -> Self {
        let (ctx, view) = fission::build::current::<UiState>();
        let palette = UiPalette::for_mode(view.state().theme_mode);
        let doctor_all = with_reducer!(ctx, RequestCommand(UiCommand::DoctorAll), request_command);
        let strict = with_reducer!(ctx, ToggleStrict, toggle_strict);
        let selected_target = view.state().selected_target;
        let selected_check = selected_target.map(|target| {
            let action = with_reducer!(
                ctx,
                RequestCommand(UiCommand::DoctorTarget(target)),
                request_command
            );
            ActionButton::new(format!("Check {}", target_label(target)), action)
                .tone(ButtonTone::Primary)
                .width(24.0)
                .into()
        });

        Column {
            gap: Some(1.0),
            children: vec![
                title_block(
                    "Doctor",
                    "Check the Rust targets, platform SDKs, device tools, and local executables needed by each platform.",
                    palette.accent,
                    palette.muted,
                ),
                KeyValueRow::new("Project", view.state().project_dir.display().to_string())
                    .into(),
                TogglePill::new("Strict exit status", view.state().strict, strict).into(),
                ActionButton::new("Check all configured tooling", doctor_all)
                    .tone(ButtonTone::Primary)
                    .width(32.0)
                    .into(),
                Text::new("Select one platform when you want a narrower check.")
                    .color(palette.muted)
                    .into(),
                TargetPicker {
                    configured_only: false,
                }
                .into(),
                selected_check.unwrap_or_else(|| Text::new("Select a target first.").into()),
            ],
            ..Default::default()
        }
        .into()
    }
}
