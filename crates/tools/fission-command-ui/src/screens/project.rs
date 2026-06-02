use super::title_block;
use crate::actions::{
    request_command, set_init_app_id, set_init_local_path, set_init_name, RequestCommand,
    SetInitAppId, SetInitLocalPath, SetInitName,
};
use crate::commands::UiCommand;
use crate::components::{ActionButton, ButtonTone, FormTextField, KeyValueRow};
use crate::state::{all_targets, target_label, UiState};
use crate::theme::UiPalette;
use fission::prelude::*;

#[derive(Clone)]
pub struct ProjectScreen;

impl From<ProjectScreen> for Widget {
    fn from(_component: ProjectScreen) -> Self {
        let (ctx, view) = fission::build::current::<UiState>();
        let palette = UiPalette::for_mode(view.state().theme_mode);
        let init = with_reducer!(ctx, RequestCommand(UiCommand::InitProject), request_command);
        let refresh = with_reducer!(ctx, RequestCommand(UiCommand::Refresh), request_command);
        let set_name = with_reducer!(ctx, SetInitName(String::new()), set_init_name);
        let set_app_id = with_reducer!(ctx, SetInitAppId(String::new()), set_init_app_id);
        let set_local_path =
            with_reducer!(ctx, SetInitLocalPath(String::new()), set_init_local_path);
        let mut target_buttons = Vec::new();
        for target in all_targets() {
            let configured = view.state().targets.contains(&target);
            if configured {
                continue;
            }
            let action = with_reducer!(
                ctx,
                RequestCommand(UiCommand::AddTarget(target)),
                request_command
            );
            target_buttons.push(
                ActionButton::new(format!("Add {}", target_label(target)), action)
                    .tone(ButtonTone::Neutral)
                    .width(20.0)
                    .into(),
            );
        }
        let target_section = if target_buttons.is_empty() {
            Text::new("All known targets are already configured.")
                .color(palette.muted)
                .into()
        } else {
            Column {
                gap: Some(1.0),
                children: target_buttons,
                ..Default::default()
            }
            .into()
        };
        Column {
            gap: Some(1.0),
            children: vec![
                title_block(
                    "Project setup",
                    "Initialise this directory and add platform scaffolds idempotently.",
                    palette.accent,
                    palette.muted,
                ),
                KeyValueRow::new("Directory", view.state().project_dir.display().to_string())
                    .into(),
                KeyValueRow::new("App id", view.state().app_id.clone()).into(),
                KeyValueRow::new("Status", view.state().project_status.clone()).into(),
                Row {
                    gap: Some(1.0),
                    children: vec![
                        FormTextField::new(
                            "cli_ui_init_name",
                            "Name override",
                            view.state().init_name.clone(),
                            "optional package name",
                            set_name,
                        )
                        .width(24.0)
                        .into(),
                        FormTextField::new(
                            "cli_ui_init_app_id",
                            "App id override",
                            view.state().init_app_id.clone(),
                            "optional app id",
                            set_app_id,
                        )
                        .width(32.0)
                        .into(),
                        FormTextField::new(
                            "cli_ui_init_local_path",
                            "Local Fission",
                            view.state().init_local_path.clone(),
                            "optional path",
                            set_local_path,
                        )
                        .width(30.0)
                        .into(),
                    ],
                    ..Default::default()
                }
                .into(),
                Row {
                    gap: Some(1.0),
                    children: vec![
                        ActionButton::new("Initialise project", init)
                            .tone(ButtonTone::Primary)
                            .into(),
                        ActionButton::new("Refresh", refresh)
                            .tone(ButtonTone::Neutral)
                            .into(),
                    ],
                    ..Default::default()
                }
                .into(),
                Text::new("Add a missing target")
                    .color(palette.accent)
                    .into(),
                target_section,
            ],
            ..Default::default()
        }
        .into()
    }
}
