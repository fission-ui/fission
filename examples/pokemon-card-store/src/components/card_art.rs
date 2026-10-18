use crate::app::StoreState;
use crate::components::layout::CARD_ART_HEIGHT;
use crate::components::palette::card_accent;
use crate::data::Card;
use fission::prelude::*;

fission_component! {
    pub CardArt<StoreState> { card: Card }
    |art, _state, cx| {
        let tokens = &cx.view.env().theme.tokens;
        let typography = &tokens.typography;
    } =>
        Container(Column(
            gap: tokens.spacing.s,
            children: [
                Text::new(art.card.type_line)
                    .size(typography.font_size_xs)
                    .line_height(typography.font_size_xs * typography.line_height_snug)
                    .weight(typography.font_weight_bold)
                    .color(tokens.colors.surface),
                Spacer(flex_grow: 1.0),
                Text::new(art.card.name)
                    .size(typography.font_size_lg)
                    .line_height(typography.font_size_lg * typography.line_height_heading)
                    .weight(typography.font_weight_bold)
                    .color(tokens.colors.surface),
            ],
        ))
        .height_length(Length::points(CARD_ART_HEIGHT))
        .padding_all(tokens.spacing.m)
        .border_radius(tokens.radii.xl)
        .bg(card_accent(art.card.accent))
}
