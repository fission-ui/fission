use crate::app::StoreState;
use crate::components::layout::{DETAIL_ART_HEIGHT, DETAIL_ART_WIDTH};
use crate::components::palette::{card_accent, SURFACE};
use crate::data::Card;
use fission::prelude::*;

pub struct DetailArt {
    pub card: Card,
}

impl From<DetailArt> for Widget {
    fn from(art: DetailArt) -> Self {
        let (_, view) = fission::build::current::<StoreState>();
        let tokens = &view.env().theme.tokens;
        let typography = &tokens.typography;

        Container::new(Column {
            gap: Some(tokens.spacing.s),
            children: widgets![
                Text::new(art.card.type_line)
                    .size(typography.body_large_size)
                    .line_height(typography.body_large_size * typography.line_height_snug)
                    .weight(typography.font_weight_bold)
                    .color(SURFACE),
                Spacer {
                    flex_grow: 1.0,
                    ..Default::default()
                },
                Text::new(art.card.name)
                    .size(typography.heading_size)
                    .line_height(typography.heading_size * typography.line_height_heading)
                    .weight(typography.font_weight_bold)
                    .color(SURFACE),
            ],
            ..Default::default()
        })
        .width_length(Length::min(vec![
            Length::percent(100.0),
            Length::points(DETAIL_ART_WIDTH),
        ]))
        .height_length(Length::points(DETAIL_ART_HEIGHT))
        .padding_all(tokens.spacing.l)
        .border_radius(tokens.radii.xxl)
        .bg(card_accent(art.card.accent))
        .into()
    }
}
