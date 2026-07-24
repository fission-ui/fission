use crate::model::FieldInspectorState;
use fission::prelude::*;

pub struct SmallButton {
    pub identifier: String,
    pub label: String,
    pub action: ActionEnvelope,
    pub variant: ButtonVariant,
}

impl SmallButton {
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

impl From<SmallButton> for Widget {
    fn from(button: SmallButton) -> Self {
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
            size: ComponentSize::Sm,
            ..Default::default()
        }
        .semantics_identifier(button.identifier)
        .into()
    }
}
