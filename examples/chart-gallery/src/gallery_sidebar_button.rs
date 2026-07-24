use crate::state::{GalleryState, SelectChart};
use fission::prelude::*;

pub(crate) struct GallerySidebarButton {
    pub(crate) action_id: ActionId,
    pub(crate) selection: SelectChart,
    pub(crate) label: &'static str,
    pub(crate) selected: bool,
    pub(crate) instance: &'static str,
}

impl From<GallerySidebarButton> for Widget {
    fn from(button: GallerySidebarButton) -> Self {
        let (_, view) = fission::build::current::<GalleryState>();
        let tokens = &view.env().theme.tokens;

        Button {
            variant: if button.selected {
                ButtonVariant::Filled
            } else {
                ButtonVariant::Ghost
            },
            on_press: Some(ActionEnvelope {
                id: button.action_id,
                payload: serde_json::to_vec(&button.selection)
                    .expect("serialize SelectChart action"),
            }),
            child: Some(
                Text::new(button.label)
                    .size(tokens.typography.body_medium_size)
                    .color(if button.selected {
                        tokens.colors.on_primary
                    } else {
                        tokens.colors.text_secondary
                    })
                    .into(),
            ),
            ..Default::default()
        }
        .semantics_identifier(format!(
            "chart-gallery.select.{}.{}.{}",
            button.instance, button.selection.0, button.selection.1
        ))
        .into()
    }
}
