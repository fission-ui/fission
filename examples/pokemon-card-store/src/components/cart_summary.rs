use crate::app::StoreState;
use crate::components::palette::{BLUE, BLUE_TEXT, TEXT_PRIMARY};
use crate::data;
use fission::prelude::*;

pub struct CartSummary;

impl From<CartSummary> for Widget {
    fn from(_: CartSummary) -> Self {
        let (_, view) = fission::build::current::<StoreState>();
        let tokens = &view.env().theme.tokens;
        let typography = &tokens.typography;
        let cart_count = view.state().cart_items.len();
        let last = view
            .state()
            .cart_items
            .last()
            .and_then(|slug| data::card_by_slug(slug))
            .map(|card| format!("Last added: {}", card.name))
            .unwrap_or_else(|| "Choose a card to exercise signed server actions.".to_string());

        Container::new(Row {
            gap: Some(tokens.spacing.s),
            wrap: ir_op::FlexWrap::Wrap,
            align_items: ir_op::AlignItems::Center,
            children: widgets![
                Text::new(format!(
                    "{} {} in the server cart",
                    cart_count,
                    if cart_count == 1 { "item" } else { "items" }
                ))
                .size(typography.body_large_size)
                .line_height(typography.body_large_size * typography.line_height_snug)
                .weight(typography.font_weight_bold)
                .color(TEXT_PRIMARY),
                Spacer {
                    flex_grow: 1.0,
                    ..Default::default()
                },
                Text::new(last)
                    .size(typography.font_size_sm)
                    .line_height(typography.font_size_sm * typography.line_height_snug)
                    .color(BLUE_TEXT),
            ],
            ..Default::default()
        })
        .padding_all(tokens.spacing.s)
        .border(BLUE.with_alpha(120), 1.0)
        .border_radius(tokens.radii.xl)
        .bg(BLUE.with_alpha(40))
        .into()
    }
}
