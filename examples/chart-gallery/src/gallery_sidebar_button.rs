use crate::state::{GalleryState, SelectChart};
use fission::prelude::*;

fission_component! {
    pub(crate) GallerySidebarButton<GalleryState> {
        action_id: ActionId,
        selection: SelectChart,
        label: &'static str,
        selected: bool,
        instance: &'static str,
    }
    |button, _state, cx| {
        let tokens = &cx.view.env().theme.tokens;
    } =>
        Button(
            variant: if button.selected { ButtonVariant::Filled } else { ButtonVariant::Ghost },
            on_press: ActionEnvelope {
                id: button.action_id,
                payload: serde_json::to_vec(&button.selection).expect("serialize SelectChart action"),
            },
            child: Text(button.label)
                .size(tokens.typography.body_medium_size)
                .color(if button.selected { tokens.colors.on_primary } else { tokens.colors.text_secondary }),
        )
        .semantics_identifier(format!(
            "chart-gallery.select.{}.{}.{}",
            button.instance, button.selection.0, button.selection.1,
        ))
}
