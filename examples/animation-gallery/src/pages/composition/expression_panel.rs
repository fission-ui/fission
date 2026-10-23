use crate::state::AnimationGalleryState;
use crate::style;
use fission::prelude::*;

const EXPRESSION_MAX_WIDTH: f32 = 820.0;

fission_component! {
    pub(super) ExpressionPanel<AnimationGalleryState> { expression: String }
    |panel, _state, cx| {
        let tokens = &cx.view.env().theme.tokens;
    } =>
        Container(
            Column(
                gap: tokens.spacing.xs,
                children: [
                    Text("Expression")
                        .size(tokens.typography.font_size_sm)
                        .color(style::text_muted()),
                    Text(panel.expression)
                        .size(tokens.typography.font_size_sm)
                        .line_height(
                            tokens.typography.font_size_sm
                                * tokens.typography.line_height_snug,
                        )
                        .color(style::text_primary())
                        .max_width(EXPRESSION_MAX_WIDTH)
                        .flex_shrink(1.0),
                ],
            ),
        )
        .padding_all(tokens.spacing.s)
        .border(style::border(), 1.0)
        .border_radius(tokens.radii.large)
        .bg(style::surface_sunken())
}
