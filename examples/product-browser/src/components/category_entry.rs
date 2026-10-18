use crate::model::ProductBrowserState;
use fission::prelude::*;

fission_component! {
    pub CategoryEntry<ProductBrowserState> {
        label: String,
        action: ActionEnvelope,
        selected: bool,
        identifier: String,
    } |component, _state, _cx| {} =>
        Button(
            id: WidgetId::explicit(&component.identifier),
            on_press: component.action,
            variant: if component.selected {
                ButtonVariant::Filled
            } else {
                ButtonVariant::Ghost
            },
            content_align: ButtonContentAlign::Start,
            child: Text(component.label).max_lines(1),
        ).semantics_identifier(component.identifier.clone())
}
