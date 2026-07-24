use crate::app::{on_add_to_cart, AddToCart, StoreState};
use crate::components::card_art::CardArt;
use crate::components::palette::{card_accent, SURFACE, TEXT_BODY, TEXT_MUTED, TEXT_PRIMARY};
use crate::data::Card;
use fission::prelude::*;

pub struct CardTile {
    pub card: Card,
}

impl From<CardTile> for Widget {
    fn from(tile: CardTile) -> Self {
        let (ctx, view) = fission::build::current::<StoreState>();
        let tokens = &view.env().theme.tokens;
        let typography = &tokens.typography;
        let accent = card_accent(tile.card.accent);
        let add = ctx.bind(
            AddToCart(tile.card.slug.to_string()),
            reduce_with!(on_add_to_cart),
        );

        Container::new(Column {
            gap: Some(tokens.spacing.s),
            children: widgets![
                CardArt {
                    card: tile.card.clone(),
                },
                Text::new(tile.card.name)
                    .size(typography.font_size_xl)
                    .line_height(typography.font_size_xl * typography.line_height_heading)
                    .weight(typography.font_weight_bold)
                    .color(TEXT_PRIMARY),
                Text::new(format!("{} - {}", tile.card.set, tile.card.rarity))
                    .size(typography.font_size_sm)
                    .line_height(typography.font_size_sm * typography.line_height_snug)
                    .weight(typography.font_weight_semibold)
                    .color(accent),
                Text::new(tile.card.description)
                    .size(typography.font_size_base)
                    .line_height(typography.font_size_base * typography.line_height_normal)
                    .color(TEXT_BODY),
                Row {
                    gap: Some(tokens.spacing.s),
                    children: widgets![
                        Text::new(format!("GBP {:.2}", tile.card.price))
                            .size(typography.font_size_lg)
                            .line_height(typography.font_size_lg * typography.line_height_heading)
                            .weight(typography.font_weight_bold)
                            .color(TEXT_PRIMARY),
                        Spacer {
                            flex_grow: 1.0,
                            ..Default::default()
                        },
                        Text::new(format!("{} left", tile.card.stock))
                            .size(typography.font_size_sm)
                            .line_height(typography.font_size_sm * typography.line_height_snug)
                            .color(TEXT_MUTED),
                    ],
                    align_items: ir_op::AlignItems::Center,
                    ..Default::default()
                },
                Row {
                    gap: Some(tokens.spacing.s),
                    children: widgets![
                        Text::new("View details")
                            .size(typography.font_size_base)
                            .line_height(typography.font_size_base * typography.line_height_snug)
                            .weight(typography.font_weight_bold)
                            .color(accent)
                            .semantics_identifier(format!("site-route:/cards/{}/", tile.card.slug)),
                        Spacer {
                            flex_grow: 1.0,
                            ..Default::default()
                        },
                        Button {
                            variant: ButtonVariant::Outline,
                            child: Some(Text::new("Add").into()),
                            on_press: Some(add),
                            ..Default::default()
                        }
                        .semantics_identifier(format!("pokemon-card-store.add.{}", tile.card.slug)),
                    ],
                    align_items: ir_op::AlignItems::Center,
                    ..Default::default()
                },
            ],
            ..Default::default()
        })
        .padding_all(tokens.spacing.m)
        .border(accent.with_alpha(90), 1.0)
        .border_radius(tokens.radii.xxl)
        .bg(SURFACE)
        .into()
    }
}
