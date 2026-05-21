use super::title_block;
use crate::ui::actions::{
    request_command, set_init_app_id, set_init_local_path, set_init_name, RequestCommand,
    SetInitAppId, SetInitLocalPath, SetInitName,
};
use crate::ui::commands::UiCommand;
use crate::ui::components::{ActionButton, ButtonTone, FormTextField, KeyValueRow};
use crate::ui::state::{all_targets, target_label, UiState};
use crate::ui::theme::UiPalette;
use fission::prelude::*;

#[derive(Clone)]
pub(crate) struct ProjectScreen;

impl Widget<UiState> for ProjectScreen {
    fn build(&self, ctx: &mut BuildCtx<UiState>, view: &View<UiState>) -> Node {
        let palette = UiPalette::for_mode(view.state.theme_mode);
        let init = with_reducer!(ctx, RequestCommand(UiCommand::InitProject), request_command);
        let refresh = with_reducer!(ctx, RequestCommand(UiCommand::Refresh), request_command);
        let set_name = with_reducer!(ctx, SetInitName(String::new()), set_init_name);
        let set_app_id = with_reducer!(ctx, SetInitAppId(String::new()), set_init_app_id);
        let set_local_path =
            with_reducer!(ctx, SetInitLocalPath(String::new()), set_init_local_path);
        let mut target_buttons = Vec::new();
        for target in all_targets() {
            let configured = view.state.targets.contains(&target);
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
                    .build(ctx, view),
            );
        }
        let target_section = if target_buttons.is_empty() {
            Text::new("All known targets are already configured.")
                .color(palette.muted)
                .into_node()
        } else {
            Column {
                gap: Some(1.0),
                children: target_buttons,
                ..Default::default()
            }
            .into_node()
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
                KeyValueRow::new("Directory", view.state.project_dir.display().to_string())
                    .build(ctx, view),
                KeyValueRow::new("App id", view.state.app_id.clone()).build(ctx, view),
                KeyValueRow::new("Status", view.state.project_status.clone()).build(ctx, view),
                Row {
                    gap: Some(1.0),
                    children: vec![
                        FormTextField::new(
                            "cli_ui_init_name",
                            "Name override",
                            view.state.init_name.clone(),
                            "optional package name",
                            set_name,
                        )
                        .width(24.0)
                        .build(ctx, view),
                        FormTextField::new(
                            "cli_ui_init_app_id",
                            "App id override",
                            view.state.init_app_id.clone(),
                            "optional app id",
                            set_app_id,
                        )
                        .width(32.0)
                        .build(ctx, view),
                        FormTextField::new(
                            "cli_ui_init_local_path",
                            "Local Fission",
                            view.state.init_local_path.clone(),
                            "optional path",
                            set_local_path,
                        )
                        .width(30.0)
                        .build(ctx, view),
                    ],
                    ..Default::default()
                }
                .into_node(),
                Row {
                    gap: Some(1.0),
                    children: vec![
                        ActionButton::new("Initialise project", init)
                            .tone(ButtonTone::Primary)
                            .build(ctx, view),
                        ActionButton::new("Refresh", refresh)
                            .tone(ButtonTone::Neutral)
                            .build(ctx, view),
                    ],
                    ..Default::default()
                }
                .into_node(),
                Text::new("Add a missing target")
                    .color(palette.accent)
                    .into_node(),
                target_section,
            ],
            ..Default::default()
        }
        .into_node()
    }
}
