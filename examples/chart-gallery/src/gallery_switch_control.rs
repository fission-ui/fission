use crate::state::GalleryState;
use fission::op::AlignItems;
use fission::prelude::*;

fission_component! {
    pub GallerySwitchControl<GalleryState> { label: &'static str, checked: bool, action: ActionEnvelope, identifier: String }
    |control, _state, cx| {
        let tokens = &cx.view.env().theme.tokens;
    } =>
        Row(
            children: [
                Text(control.label).color(tokens.colors.text_primary),
                Switch(
                    id: WidgetId::explicit(&control.identifier),
                    semantics_identifier: control.identifier,
                    checked: control.checked,
                    on_toggle: control.action.clone(),
                ),
            ],
            gap: tokens.spacing.s,
            align_items: AlignItems::Center,
        )
}
