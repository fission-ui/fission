use crate::model::FieldInspectorState;
use fission::prelude::*;

pub struct ActionButton {
    pub identifier: String,
    pub label: String,
    pub action: ActionEnvelope,
    pub variant: ButtonVariant,
}

impl ActionButton {
    pub fn new(
        identifier: impl Into<String>,
        label: impl Into<String>,
        action: ActionEnvelope,
        variant: ButtonVariant,
    ) -> Self {
        Self {
            identifier: identifier.into(),
            label: label.into(),
            action,
            variant,
        }
    }
}

impl From<ActionButton> for Widget {
    fn from(button: ActionButton) -> Self {
        let (_, view) = fission::build::current::<FieldInspectorState>();
        let typography = &view.env().theme.tokens.typography;

        Button {
            child: Some(
                Text::new(button.label)
                    .weight(typography.font_weight_bold)
                    .into(),
            ),
            on_press: Some(button.action),
            variant: button.variant,
            ..Default::default()
        }
        .semantics_identifier(button.identifier)
        .into()
    }
}
