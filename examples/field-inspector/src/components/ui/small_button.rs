use crate::model::FieldInspectorState;
use fission::prelude::*;

fission_component! {
    pub SmallButton<FieldInspectorState> { identifier: String, label: String, action: ActionEnvelope, variant: ButtonVariant }
    |button, _state, cx| {
        let typography = &cx.view.env().theme.tokens.typography;
    } =>
        Button(
            child: Text(button.label).weight(typography.font_weight_bold),
            on_press: button.action,
            variant: button.variant,
            size: ComponentSize::Sm,
        ).semantics_identifier(button.identifier.clone())
}
