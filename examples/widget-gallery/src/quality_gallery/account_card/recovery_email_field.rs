use super::layout_id;
use fission::prelude::*;

pub(super) struct RecoveryEmailField {
    pub compact: bool,
    pub value: String,
    pub on_input: ActionEnvelope,
}

impl From<RecoveryEmailField> for Widget {
    fn from(field: RecoveryEmailField) -> Self {
        FormControl {
            id: Some(layout_id("quality-gallery.recovery-email", field.compact)),
            label: Some("Recovery email".into()),
            child: TextInput {
                id: Some(layout_id(
                    "quality-gallery.recovery-email.input",
                    field.compact,
                )),
                semantics_identifier: Some("quality-gallery.input.recovery-email".into()),
                value: field.value,
                on_input: Some(field.on_input),
                size: ComponentSize::Md,
                ..Default::default()
            }
            .into(),
            error: Some("Enter a complete email address.".into()),
            helper: None,
            required: false,
        }
        .into()
    }
}
