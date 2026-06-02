use crate::actions::{cancel_dialog, confirm_dialog, CancelDialog, ConfirmDialog};
use crate::components::{ActionButton, ButtonTone};
use crate::state::{UiDialog, UiState};
use crate::theme::UiPalette;
use fission::op::{AlignItems, JustifyContent};
use fission::prelude::*;

#[derive(Clone)]
pub struct ConfirmationDialog;

impl From<ConfirmationDialog> for Widget {
    fn from(_component: ConfirmationDialog) -> Self {
        let (ctx, view) = fission::build::current::<UiState>();
        let palette = UiPalette::for_mode(view.state().theme_mode);
        let Some(dialog) = &view.state().pending_dialog else {
            return Spacer::default().into();
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

        Container::new(Row {
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
        })
        .width(view.env().viewport_size.width)
        .height(view.env().viewport_size.height)
        .bg(palette.background)
        .into()
    }
}
fn dialog_card(
    title: &str,
    message: &str,
    confirm_label: &str,
    tone: ButtonTone,
    confirm: ActionEnvelope,
    cancel: ActionEnvelope,
    _ctx: BuildCtxHandle<UiState>,
    view: ViewHandle<UiState>,
) -> Widget {
    let palette = UiPalette::for_mode(view.state().theme_mode);
    Container::new(Column {
        gap: Some(1.0),
        children: vec![
            Text::new(title).color(palette.accent).into(),
            Text::new(message).color(palette.text).into(),
            Text::new("Use Tab to choose, Enter to confirm, or Cancel to return.")
                .color(palette.muted)
                .into(),
            Row {
                gap: Some(1.0),
                children: vec![
                    ActionButton::new(confirm_label, confirm)
                        .tone(tone)
                        .width(14.0)
                        .into(),
                    ActionButton::new("Cancel", cancel)
                        .tone(ButtonTone::Neutral)
                        .width(14.0)
                        .into(),
                ],
                ..Default::default()
            }
            .into(),
        ],
        ..Default::default()
    })
    .width(72.0)
    .padding([2.0, 2.0, 1.0, 1.0])
    .bg(palette.surface)
    .border(palette.accent, 1.0)
    .into()
}
