use crate::state::GalleryState;
use fission::prelude::*;

pub struct ShowcaseMetricCard {
    title: &'static str,
    value: &'static str,
    detail: &'static str,
    accent: Color,
}

impl ShowcaseMetricCard {
    pub fn new(
        title: &'static str,
        value: &'static str,
        detail: &'static str,
        accent: Color,
    ) -> Self {
        Self {
            title,
            value,
            detail,
            accent,
        }
    }
}

impl From<ShowcaseMetricCard> for Widget {
    fn from(card: ShowcaseMetricCard) -> Self {
        let (_, view) = fission::build::current::<GalleryState>();
        let tokens = &view.env().theme.tokens;

        Container::new(Column {
            children: widgets![
                Text::new(card.title)
                    .size(tokens.typography.font_size_sm)
                    .color(card.accent),
                Text::new(card.value)
                    .size(tokens.typography.font_size_xl)
                    .color(tokens.colors.text_primary),
                Text::new(card.detail)
                    .size(tokens.typography.body_medium_size)
                    .color(tokens.colors.text_secondary),
            ],
            gap: Some(tokens.spacing.s),
            ..Default::default()
        })
        .width_length(Length::percent(100.0))
        .padding_all(tokens.spacing.m)
        .border_radius(tokens.radii.xl)
        .bg(tokens.colors.surface)
        .border(tokens.colors.border, 1.0)
        .into()
    }
}
