use super::section::SectionHeading;
use crate::model::settings::{set_signature, set_signature_editing};
use crate::model::{InboxState, SetSignature, SetSignatureEditing};
use fission::prelude::*;

fission_component! {
/// The signature appended to new emails.
    pub(super) SignatureSettings<InboxState> |_settings, state, cx| {
        let stop_editing = cx.bind(
            SetSignatureEditing(false),
            reduce_with!(set_signature_editing),
        );
    } =>
        VStack(
            spacing: cx.view.env().theme.tokens.spacing.m,
            children: [
                SectionHeading("settings.signature.title"),
                FormControl(
                    label: Some(cx.tr("settings.signature.label")),
                    helper: Some(cx.tr("settings.signature.helper")),
                    child: Editable(
                        id: Some(WidgetId::explicit("settings_signature_editor")),
                        value: state.signature.clone(),
                        placeholder: cx.tr("settings.signature.placeholder"),
                        is_editing: state.signature_editing,
                        on_input: cx.bind(
                            SetSignature(state.signature.clone()),
                            reduce_with!(set_signature),
                        ),
                        on_submit: stop_editing.clone(),
                        on_edit: cx.bind(
                            SetSignatureEditing(true),
                            reduce_with!(set_signature_editing),
                        ),
                        on_cancel: stop_editing.clone(),
                    ),
                ),
                Button(
                    variant: ButtonVariant::Outline,
                    child: Text(TextContent::Key("settings.signature.save".into())),
                    on_press: stop_editing,
                ),
            ],
        )
}
