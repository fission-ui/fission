use crate::state::{composition_type_name_for_path, AnimationGalleryState};
use crate::style::{BORDER, INK, MUTED, SURFACE};
use fission::prelude::*;

const EXPRESSION_MAX_WIDTH: f32 = 650.0;

pub(super) struct ComposerExpression<'a> {
    pub path: &'a str,
    pub expression: &'a str,
}

impl From<ComposerExpression<'_>> for Widget {
    fn from(expression: ComposerExpression<'_>) -> Self {
        let (_, view) = fission::build::current::<AnimationGalleryState>();
        let tokens = &view.env().theme.tokens;
        let typography = &tokens.typography;

        Container::new(Column {
            gap: Some(tokens.spacing.xs),
            children: widgets![
                Text::new(format!(
                    "{} expression",
                    composition_type_name_for_path(expression.path)
                ))
                .size(typography.font_size_sm)
                .color(MUTED),
                Text::new(expression.expression)
                    .size(typography.font_size_sm)
                    .line_height(typography.font_size_sm * typography.line_height_snug,)
                    .color(INK)
                    .max_width(EXPRESSION_MAX_WIDTH),
            ],
            ..Default::default()
        })
        .padding_all(tokens.spacing.s)
        .border(BORDER, 1.0)
        .border_radius(tokens.radii.large)
        .bg(SURFACE)
        .into()
    }
}
