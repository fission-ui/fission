use crate::state::AnimationGalleryState;
use crate::style::{color, BORDER, INK, MUTED};
use fission::prelude::*;

const EXPRESSION_MAX_WIDTH: f32 = 820.0;

pub(super) struct ExpressionPanel<'a> {
    pub expression: &'a str,
}

impl From<ExpressionPanel<'_>> for Widget {
    fn from(panel: ExpressionPanel<'_>) -> Self {
        let (_, view) = fission::build::current::<AnimationGalleryState>();
        let tokens = &view.env().theme.tokens;
        let typography = &tokens.typography;

        Container::new(Column {
            gap: Some(tokens.spacing.xs),
            children: widgets![
                Text::new("Expression")
                    .size(typography.font_size_sm)
                    .color(MUTED),
                Text::new(panel.expression)
                    .size(typography.font_size_sm)
                    .line_height(typography.font_size_sm * typography.line_height_snug,)
                    .color(INK)
                    .max_width(EXPRESSION_MAX_WIDTH)
                    .flex_shrink(1.0),
            ],
            ..Default::default()
        })
        .padding_all(tokens.spacing.s)
        .border(BORDER, 1.0)
        .border_radius(tokens.radii.large)
        .bg(color(249, 251, 255, 255))
        .into()
    }
}
