use crate::app::StoreState;
use crate::components::palette::{BLUE_TEXT, RED, SURFACE, TEXT_BODY, TEXT_PRIMARY};
use fission::prelude::*;

pub struct NotFound {
    pub slug: String,
}

impl From<NotFound> for Widget {
    fn from(missing: NotFound) -> Self {
        let (_, view) = fission::build::current::<StoreState>();
        let tokens = &view.env().theme.tokens;
        let typography = &tokens.typography;

        Container::new(Column {
            gap: Some(tokens.spacing.s),
            children: widgets![
                Text::new("Card not found")
                    .size(typography.heading_size)
                    .line_height(typography.heading_size * typography.line_height_heading)
                    .weight(typography.font_weight_bold)
                    .color(TEXT_PRIMARY),
                Text::new(format!("No card route exists for `{}`.", missing.slug))
                    .size(typography.body_large_size)
                    .line_height(typography.body_large_size * typography.line_height_normal)
                    .color(TEXT_BODY),
                Text::new("Back to catalogue")
                    .size(typography.body_medium_size)
                    .line_height(typography.body_medium_size * typography.line_height_snug)
                    .weight(typography.font_weight_bold)
                    .color(BLUE_TEXT)
                    .semantics_identifier("site-route:/"),
            ],
            ..Default::default()
        })
        .padding_all(tokens.spacing.l)
        .border(RED.with_alpha(120), 1.0)
        .border_radius(tokens.radii.xxl)
        .bg(SURFACE)
        .into()
    }
}
