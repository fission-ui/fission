use crate::ui::actions::{cancel_dialog, confirm_dialog, CancelDialog, ConfirmDialog};
use crate::ui::components::{ActionButton, ButtonTone};
use crate::ui::state::{UiDialog, UiState};
use crate::ui::theme::UiPalette;
use fission::ir::op::{AlignItems, JustifyContent};
use fission::prelude::*;

#[derive(Clone)]
pub(crate) struct ConfirmationDialog;

impl Widget<UiState> for ConfirmationDialog {
    fn build(&self, ctx: &mut BuildCtx<UiState>, view: &View<UiState>) -> Node {
        let palette = UiPalette::for_mode(view.state.theme_mode);
        let Some(dialog) = &view.state.pending_dialog else {
            return Spacer::default().into_node();
        };
        let (title, message, confirm_label, tone) = match dialog {
            UiDialog::Command { title, message, .. } => {
                (title.as_str(), message.as_str(), "Run", ButtonTone::Primary)
            }
            UiDialog::Exit { title, message } => (
                title.as_str(),
                message.as_str(),
                "Exit",
                ButtonTone::Warning,
            ),
        };
        let confirm = with_reducer!(ctx, ConfirmDialog, confirm_dialog);
        let cancel = with_reducer!(ctx, CancelDialog, cancel_dialog);

        Container::new(
            Row {
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                children: vec![dialog_card(
                    title,
                    message,
                    confirm_label,
                    tone,
                    confirm,
                    cancel,
                    ctx,
                    view,
                )],
                ..Default::default()
            }
            .into_node(),
        )
        .width(view.env.viewport_size.width)
        .height(view.env.viewport_size.height)
        .bg(palette.background)
        .into_node()
    }
}

fn dialog_card(
    title: &str,
    message: &str,
    confirm_label: &str,
    tone: ButtonTone,
    confirm: ActionEnvelope,
    cancel: ActionEnvelope,
    ctx: &mut BuildCtx<UiState>,
    view: &View<UiState>,
) -> Node {
    let palette = UiPalette::for_mode(view.state.theme_mode);
    Container::new(
        Column {
            gap: Some(1.0),
            children: vec![
                Text::new(title).color(palette.accent).into_node(),
                Text::new(message).color(palette.text).into_node(),
                Text::new("Use Tab to choose, Enter to confirm, or Cancel to return.")
                    .color(palette.muted)
                    .into_node(),
                Row {
                    gap: Some(1.0),
                    children: vec![
                        ActionButton::new(confirm_label, confirm)
                            .tone(tone)
                            .width(14.0)
                            .build(ctx, view),
                        ActionButton::new("Cancel", cancel)
                            .tone(ButtonTone::Neutral)
                            .width(14.0)
                            .build(ctx, view),
                    ],
                    ..Default::default()
                }
                .into_node(),
            ],
            ..Default::default()
        }
        .into_node(),
    )
    .width(72.0)
    .padding([2.0, 2.0, 1.0, 1.0])
    .bg(palette.surface)
    .border(palette.accent, 1.0)
    .into_node()
}
