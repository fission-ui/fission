use crate::model::FieldInspectorState;
use fission::prelude::*;

fission_component! {
    pub PanelCard<FieldInspectorState> { child: Widget }
    |panel, _state, cx| {
        let tokens = &cx.view.env().theme.tokens;
        let mut container = Container::new(panel.child.clone())
            .bg(tokens.colors.surface)
            .border(tokens.colors.border.with_alpha(150), 1.0)
            .border_radius(tokens.radii.xxl)
            .padding_all(tokens.spacing.m);
        if let Some(shadow) = tokens.elevations.level2 { container = container.shadow(shadow); }
    } => container
}
