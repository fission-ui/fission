use crate::model::FieldInspectorState;
use fission::prelude::*;

pub struct PanelCard {
    pub child: Widget,
}

impl PanelCard {
    pub fn new(child: impl Into<Widget>) -> Self {
        Self {
            child: child.into(),
        }
    }
}

impl From<PanelCard> for Widget {
    fn from(panel: PanelCard) -> Self {
        let (_, view) = fission::build::current::<FieldInspectorState>();
        let tokens = &view.env().theme.tokens;
        let mut container = Container::new(panel.child)
            .bg(tokens.colors.surface)
            .border(tokens.colors.border.with_alpha(150), 1.0)
            .border_radius(tokens.radii.xxl)
            .padding_all(tokens.spacing.m);

        if let Some(shadow) = tokens.elevations.level2 {
            container = container.shadow(shadow);
        }

        container.into()
    }
}
