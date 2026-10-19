use super::section::SectionHeading;
use crate::model::InboxState;
use fission::prelude::*;

fission_component! {
/// The signature appended to new emails.
    pub(super) SignatureSettings<InboxState> |_settings, state, cx| {} =>
        VStack(
            spacing: cx.view.env().theme.tokens.spacing.m,
            children: [
                SectionHeading("settings.signature.title"),
                FormControl(
                    label: Some(cx.tr("settings.signature.label")),
                    helper: Some(cx.tr("settings.signature.helper")),
                    child: Editable(
                        key: "signature",
                        id: Some(WidgetId::explicit("settings_signature_editor")),
                        value: state.signature.clone(),
                        placeholder: cx.tr("settings.signature.placeholder"),
                        is_editing: state.signature_editing,
                        on_input: |_settings, state, cx| {
                            if let Some(change) = cx.input.text_change() {
                                state.signature = change.new_text.clone();
                            }
                        },
                        on_submit: |_settings, state, _cx| state.signature_editing = false,
                        on_edit: |_settings, state, _cx| state.signature_editing = true,
                        on_cancel: |_settings, state, _cx| state.signature_editing = false,
                    ),
                ),
                Button(
                    key: "save-signature",
                    variant: ButtonVariant::Outline,
                    child: Text(TextContent::Key("settings.signature.save".into())),
                    on_press: |_settings, state, _cx| state.signature_editing = false,
                ),
            ],
        )
}
