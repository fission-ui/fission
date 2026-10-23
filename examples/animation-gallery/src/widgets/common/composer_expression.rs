use crate::state::{composition_type_name_for_path, AnimationGalleryState};
use crate::style;
use fission::prelude::*;

const EXPRESSION_MAX_WIDTH: f32 = 650.0;

fission_component! {
    pub(super) ComposerExpression<AnimationGalleryState> { path: String, expression: String }
    |expression, _state, cx| {
        let tokens = &cx.view.env().theme.tokens;
        let typography = &tokens.typography;
    } =>
        Container(
            Column(
                gap: tokens.spacing.xs,
                children: [
                Text(format!(
                    "{} expression",
                    composition_type_name_for_path(&expression.path)
                ))
                .size(typography.font_size_sm)
                .color(style::text_muted()),
                Text(expression.expression.clone())
                    .size(typography.font_size_sm)
                    .line_height(typography.font_size_sm * typography.line_height_snug,)
                    .color(style::text_primary())
                    .max_width(EXPRESSION_MAX_WIDTH),
                ],
            ),
        )
        .padding_all(tokens.spacing.s)
        .border(style::border(), 1.0)
        .border_radius(tokens.radii.large)
        .bg(style::surface())
}
