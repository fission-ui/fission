use super::section::SectionHeading;
use crate::model::settings::{set_signature, set_signature_editing};
use crate::model::{InboxState, SetSignature, SetSignatureEditing};
use fission::core::ui::{Button, ButtonVariant, Text, TextContent, Widget};
use fission::core::{reduce_with, WidgetId};
use fission::widgets::{Editable, FormControl, VStack};

/// The signature appended to new emails.
pub(super) struct SignatureSettings;

impl From<SignatureSettings> for Widget {
    fn from(_: SignatureSettings) -> Self {
        let (ctx, view) = fission::build::current::<InboxState>();
        let state = view.state();
        let stop_editing = ctx.bind(
            SetSignatureEditing(false),
            reduce_with!(set_signature_editing),
        );

        VStack {
            spacing: Some(view.env().theme.tokens.spacing.m),
            children: vec![
                SectionHeading {
                    key: "settings.signature.title",
                }
                .into(),
                FormControl {
                    id: None,
                    label: Some(view.tr("settings.signature.label")),
                    required: false,
                    error: None,
                    helper: Some(view.tr("settings.signature.helper")),
                    child: Editable {
                        id: Some(WidgetId::explicit("settings_signature_editor")),
                        value: state.signature.clone(),
                        placeholder: view.tr("settings.signature.placeholder"),
                        is_editing: state.signature_editing,
                        on_input: Some(ctx.bind(
                            SetSignature(state.signature.clone()),
                            reduce_with!(set_signature),
                        )),
                        on_submit: Some(stop_editing.clone()),
                        on_edit: Some(ctx.bind(
                            SetSignatureEditing(true),
                            reduce_with!(set_signature_editing),
                        )),
                        on_cancel: Some(stop_editing.clone()),
                    }
                    .into(),
                }
                .into(),
                Button {
                    variant: ButtonVariant::Outline,
                    child: Some(
                        Text::new(TextContent::Key("settings.signature.save".into())).into(),
                    ),
                    on_press: Some(stop_editing),
                    ..Default::default()
                }
                .into(),
            ],
        }
        .into()
    }
}
