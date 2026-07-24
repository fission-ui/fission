use crate::state::GalleryState;
use fission::op::AlignItems;
use fission::prelude::*;

pub struct GallerySwitchControl {
    pub label: &'static str,
    pub checked: bool,
    pub action: ActionEnvelope,
    pub identifier: String,
}

impl From<GallerySwitchControl> for Widget {
    fn from(control: GallerySwitchControl) -> Self {
        let (_, view) = fission::build::current::<GalleryState>();
        let tokens = &view.env().theme.tokens;
        Row {
            children: widgets![
                Text::new(control.label).color(tokens.colors.text_primary),
                fission::widgets::Switch {
                    id: Some(WidgetId::explicit(&control.identifier)),
                    semantics_identifier: Some(control.identifier),
                    checked: control.checked,
                    on_toggle: Some(control.action),
                    ..Default::default()
                },
            ],
            gap: Some(tokens.spacing.s),
            align_items: AlignItems::Center,
            ..Default::default()
        }
        .into()
    }
}
