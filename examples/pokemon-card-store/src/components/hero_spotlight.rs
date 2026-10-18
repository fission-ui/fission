use crate::app::StoreState;
use crate::components::layout::HERO_SPOTLIGHT_WIDTH;
use crate::components::palette::card_accent;
use crate::data;
use fission::prelude::*;

fission_component! {
    pub HeroSpotlight<StoreState> {}
    |_spotlight, _state, cx| {
        let tokens = &cx.view.env().theme.tokens;
        let typography = &tokens.typography;
        let card = &data::cards()[0];

    } =>
        Container(Column(
            gap: tokens.spacing.m,
            children: [
                Text::new("Featured card")
                    .size(typography.font_size_sm)
                    .line_height(typography.font_size_sm * typography.line_height_snug)
                    .weight(typography.font_weight_bold)
                    .color(tokens.colors.info),
                Text::new(card.name)
                    .size(typography.heading_size)
                    .line_height(typography.heading_size * typography.line_height_heading)
                    .weight(typography.font_weight_bold)
                    .color(tokens.colors.info.with_alpha(245)),
                Text::new(card.description)
                    .size(typography.body_medium_size)
                    .line_height(typography.body_medium_size * typography.line_height_normal)
                    .color(tokens.colors.info.with_alpha(215)),
                Text::new(format!("GBP {:.2} - {} left", card.price, card.stock))
                    .size(typography.body_large_size)
                    .line_height(typography.body_large_size * typography.line_height_snug)
                    .weight(typography.font_weight_bold)
                    .color(tokens.colors.info.with_alpha(235)),
            ],
        ))
        .width_length(Length::min(vec![
            Length::percent(100.0),
            Length::points(HERO_SPOTLIGHT_WIDTH),
        ]))
        .padding_all(tokens.spacing.l)
        .border(tokens.colors.info.with_alpha(150), 1.0)
        .border_radius(tokens.radii.xxl)
        .bg(card_accent(card.accent).with_alpha(70))
}
