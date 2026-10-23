use super::layout_id;
use crate::GalleryState;
use fission::prelude::*;

fission_component! {
    pub(super) RecoveryEmailField<GalleryState> {
        compact: bool,
        value: String,
        on_input: ActionEnvelope,
    }
    |field, _state, _cx| {} =>
        FormControl(
            id: layout_id("quality-gallery.recovery-email", field.compact),
            label: "Recovery email".to_string(),
            child: TextInput(
                id: layout_id(
                    "quality-gallery.recovery-email.input",
                    field.compact,
                ),
                semantics_identifier: "quality-gallery.input.recovery-email".to_string(),
                value: field.value.clone(),
                on_input: field.on_input.clone(),
                size: ComponentSize::Md,
            ),
            error: "Enter a complete email address.".to_string(),
            helper: None,
            required: false,
        )
}
