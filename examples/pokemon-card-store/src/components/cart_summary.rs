use crate::app::StoreState;
use crate::data;
use fission::prelude::*;

fission_component! {
    pub CartSummary<StoreState> {}
    |_summary, state, cx| {
        let tokens = &cx.view.env().theme.tokens;
        let typography = &tokens.typography;
        let cart_count = state.cart_items.len();
        let last = state
            .cart_items
            .last()
            .and_then(|slug| data::card_by_slug(slug))
            .map(|card| format!("Last added: {}", card.name))
            .unwrap_or_else(|| "Choose a card to exercise signed server actions.".to_string());

    } =>
        Container(Row(
            gap: tokens.spacing.s,
            wrap: ir_op::FlexWrap::Wrap,
            align_items: ir_op::AlignItems::Center,
            children: [
                Text::new(format!(
                    "{} {} in the server cart",
                    cart_count,
                    if cart_count == 1 { "item" } else { "items" }
                ))
                .size(typography.body_large_size)
                .line_height(typography.body_large_size * typography.line_height_snug)
                .weight(typography.font_weight_bold)
                .color(tokens.colors.text_primary),
                Spacer(flex_grow: 1.0),
                Text::new(last)
                    .size(typography.font_size_sm)
                    .line_height(typography.font_size_sm * typography.line_height_snug)
                    .color(tokens.colors.text_link),
            ],
        ))
        .padding_all(tokens.spacing.s)
        .border(tokens.colors.primary.with_alpha(120), 1.0)
        .border_radius(tokens.radii.xl)
        .bg(tokens.colors.primary.with_alpha(40))
}
