use crate::model::FieldInspectorState;
use fission::prelude::*;

pub struct SoftPanel {
    pub child: Widget,
}

impl SoftPanel {
    pub fn new(child: impl Into<Widget>) -> Self {
        Self {
            child: child.into(),
        }
    }
}

impl From<SoftPanel> for Widget {
    fn from(panel: SoftPanel) -> Self {
        let (_, view) = fission::build::current::<FieldInspectorState>();
        let tokens = &view.env().theme.tokens;

        Container::new(panel.child)
            .bg(tokens.colors.background.with_alpha(170))
            .border(tokens.colors.border.with_alpha(120), 1.0)
            .border_radius(tokens.radii.xl)
            .padding_all(tokens.spacing.s)
            .into()
    }
}
