use crate::model::FieldInspectorState;
use fission::prelude::*;

fission_component! {
    pub SoftPanel<FieldInspectorState> { child: Widget }
    |panel, _state, cx| {
        let tokens = &cx.view.env().theme.tokens;
    } =>
        Container(panel.child)
            .bg(tokens.colors.background.with_alpha(170))
            .border(tokens.colors.border.with_alpha(120), 1.0)
            .border_radius(tokens.radii.xl)
            .padding_all(tokens.spacing.s)
}
