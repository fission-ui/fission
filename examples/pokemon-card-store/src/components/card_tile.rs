use crate::app::{on_add_to_cart, AddToCart, StoreState};
use crate::components::card_art::CardArt;
use crate::components::palette::card_accent;
use crate::data::Card;
use fission::prelude::*;

fission_component! {
    pub CardTile<StoreState> { card: Card }
    |tile, _state, cx| {
        let tokens = &cx.view.env().theme.tokens;
        let typography = &tokens.typography;
        let accent = card_accent(tile.card.accent);
        let add = cx.build.bind(
            AddToCart(tile.card.slug.to_string()),
            reduce_with!(on_add_to_cart),
        );

    } =>
        Container(Column(
            gap: tokens.spacing.s,
            children: [
                CardArt(tile.card.clone()),
                Text::new(tile.card.name)
                    .size(typography.font_size_xl)
                    .line_height(typography.font_size_xl * typography.line_height_heading)
                    .weight(typography.font_weight_bold)
                    .color(tokens.colors.text_primary),
                Text::new(format!("{} - {}", tile.card.set, tile.card.rarity))
                    .size(typography.font_size_sm)
                    .line_height(typography.font_size_sm * typography.line_height_snug)
                    .weight(typography.font_weight_semibold)
                    .color(accent),
                Text::new(tile.card.description)
                    .size(typography.font_size_base)
                    .line_height(typography.font_size_base * typography.line_height_normal)
                    .color(tokens.colors.text_secondary),
                Row(
                    gap: tokens.spacing.s,
                    children: [
                        Text::new(format!("GBP {:.2}", tile.card.price))
                            .size(typography.font_size_lg)
                            .line_height(typography.font_size_lg * typography.line_height_heading)
                            .weight(typography.font_weight_bold)
                            .color(tokens.colors.text_primary),
                        Spacer(flex_grow: 1.0),
                        Text::new(format!("{} left", tile.card.stock))
                            .size(typography.font_size_sm)
                            .line_height(typography.font_size_sm * typography.line_height_snug)
                            .color(tokens.colors.text_muted),
                    ],
                    align_items: ir_op::AlignItems::Center,
                ),
                Row(
                    gap: tokens.spacing.s,
                    children: [
                        Text::new("View details")
                            .size(typography.font_size_base)
                            .line_height(typography.font_size_base * typography.line_height_snug)
                            .weight(typography.font_weight_bold)
                            .color(accent)
                            .semantics_identifier(format!("site-route:/cards/{}/", tile.card.slug)),
                        Spacer(flex_grow: 1.0),
                        Button(
                            variant: ButtonVariant::Outline,
                            child: Text("Add"),
                            on_press: add,
                        )
                        .semantics_identifier(format!("pokemon-card-store.add.{}", tile.card.slug)),
                    ],
                    align_items: ir_op::AlignItems::Center,
                ),
            ],
        ))
        .padding_all(tokens.spacing.m)
        .border(accent.with_alpha(90), 1.0)
        .border_radius(tokens.radii.xxl)
        .bg(tokens.colors.surface)
}
