use crate::app::StoreState;
use crate::components::layout::CARD_ART_HEIGHT;
use crate::components::palette::{card_accent, SURFACE};
use crate::data::Card;
use fission::prelude::*;

pub struct CardArt {
    pub card: Card,
}

impl From<CardArt> for Widget {
    fn from(art: CardArt) -> Self {
        let (_, view) = fission::build::current::<StoreState>();
        let tokens = &view.env().theme.tokens;
        let typography = &tokens.typography;

        Container::new(Column {
            gap: Some(tokens.spacing.s),
            children: widgets![
                Text::new(art.card.type_line)
                    .size(typography.font_size_xs)
                    .line_height(typography.font_size_xs * typography.line_height_snug)
                    .weight(typography.font_weight_bold)
                    .color(SURFACE),
                Spacer {
                    flex_grow: 1.0,
                    ..Default::default()
                },
                Text::new(art.card.name)
                    .size(typography.font_size_lg)
                    .line_height(typography.font_size_lg * typography.line_height_heading)
                    .weight(typography.font_weight_bold)
                    .color(SURFACE),
            ],
            ..Default::default()
        })
        .height_length(Length::points(CARD_ART_HEIGHT))
        .padding_all(tokens.spacing.m)
        .border_radius(tokens.radii.xl)
        .bg(card_accent(art.card.accent))
        .into()
    }
}
