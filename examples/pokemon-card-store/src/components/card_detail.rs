use crate::app::{on_add_to_cart, AddToCart, StoreState};
use crate::components::cart_summary::CartSummary;
use crate::components::detail_art::DetailArt;
use crate::components::layout::DETAIL_ART_WIDTH;
use crate::components::palette::{card_accent, SURFACE, TEXT_BODY, TEXT_PRIMARY};
use crate::data::Card;
use fission::prelude::*;

pub struct CardDetail {
    pub card: Card,
}

impl From<CardDetail> for Widget {
    fn from(detail: CardDetail) -> Self {
        let (ctx, view) = fission::build::current::<StoreState>();
        let tokens = &view.env().theme.tokens;
        let typography = &tokens.typography;
        let accent = card_accent(detail.card.accent);
        let add = ctx.bind(
            AddToCart(detail.card.slug.to_string()),
            reduce_with!(on_add_to_cart),
        );

        Column {
            gap: Some(tokens.spacing.l),
            children: widgets![
                Text::new("Card details")
                    .size(typography.font_size_base)
                    .line_height(typography.font_size_base * typography.line_height_snug)
                    .weight(typography.font_weight_bold)
                    .color(accent)
                    .semantics_identifier("site-route:/"),
                Container::new(Grid {
                    columns: vec![GridTrack::auto_fit(GridTrack::minmax(
                        GridTrack::Points(DETAIL_ART_WIDTH),
                        GridTrack::Fr(1.0),
                    ))],
                    rows: vec![GridTrack::Auto],
                    column_gap: Some(tokens.spacing.l),
                    row_gap: Some(tokens.spacing.l),
                    children: widgets![
                        DetailArt {
                            card: detail.card.clone(),
                        },
                        Column {
                            gap: Some(tokens.spacing.m),
                            children: widgets![
                                Text::new(detail.card.name)
                                    .size(typography.heading1_size)
                                    .line_height(
                                        typography.heading1_size * typography.line_height_heading
                                    )
                                    .weight(typography.font_weight_bold)
                                    .color(TEXT_PRIMARY),
                                Text::new(format!(
                                    "{} - {} - {}",
                                    detail.card.set, detail.card.rarity, detail.card.type_line
                                ))
                                .size(typography.body_large_size)
                                .line_height(
                                    typography.body_large_size * typography.line_height_snug
                                )
                                .weight(typography.font_weight_bold)
                                .color(accent),
                                Text::new(detail.card.description)
                                    .size(typography.body_large_size)
                                    .line_height(
                                        typography.body_large_size * typography.line_height_normal
                                    )
                                    .color(TEXT_BODY),
                                Text::new(format!(
                                    "GBP {:.2} - {} currently in stock",
                                    detail.card.price, detail.card.stock
                                ))
                                .size(typography.font_size_xl)
                                .line_height(
                                    typography.font_size_xl * typography.line_height_heading
                                )
                                .weight(typography.font_weight_bold)
                                .color(TEXT_PRIMARY),
                                Button {
                                    variant: ButtonVariant::Filled,
                                    child: Some(Text::new("Add this card to basket").into()),
                                    on_press: Some(add),
                                    ..Default::default()
                                }
                                .semantics_identifier(format!(
                                    "pokemon-card-store.add.{}",
                                    detail.card.slug
                                )),
                                CartSummary,
                            ],
                            ..Default::default()
                        },
                    ],
                    ..Default::default()
                })
                .padding_all(tokens.spacing.l)
                .border(accent.with_alpha(120), 1.0)
                .border_radius(tokens.radii.xxl)
                .bg(SURFACE),
            ],
            ..Default::default()
        }
        .into()
    }
}
