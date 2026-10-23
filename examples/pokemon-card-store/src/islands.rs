use fission::prelude::*;
use fission::site::{run_browser_island, BrowserIslandApp};

#[derive(Debug, Default, Clone)]
pub struct BrowserCartState {
    count: u32,
}

impl GlobalState for BrowserCartState {}

/// What one Charizard Holo costs in the island's cart.
const CHARIZARD_PRICE: f32 = 249.00;

fission_component! {
    #[derive(Clone)] pub CartDrawerIsland<BrowserCartState> {}
    |_component, state, cx| { let tokens = &cx.view.env().theme.tokens; } =>
        Container(Column(
            gap: tokens.spacing.m,
            children: [
                IslandStatus(state.count),
                IslandCartSummary(state.count),
                IslandCheckoutRow(state.count),
                IslandLastEvent(state.count),
                IslandCountProbe(state.count),
            ],
        ))
}

fission_component! {
/// Whether the island has handled a reducer event yet.
    IslandStatus<BrowserCartState> { count: u32 }
    |status, _state, cx| {
        let typography = &cx.view.env().theme.tokens.typography;
        let colors = &cx.view.env().theme.tokens.colors;
        let text = if status.count == 0 {
            "Island bridge ready"
        } else {
            "Island handled browser-side reducer event"
        };
    } =>
        Text(text)
            .size(typography.body_medium_size)
            .line_height(typography.body_medium_size * typography.line_height_normal)
            .weight(typography.font_weight_bold)
            .color(colors.warning)
            .semantics_identifier("island-status:cart-drawer")
}

fission_component! {
/// The staged line and item count, in a card outlined in the accent colour.
    IslandCartSummary<BrowserCartState> { count: u32 }
    |summary, _state, cx| {
        let tokens = &cx.view.env().theme.tokens;
        let typography = &tokens.typography;
        let count = summary.count;
        let line = if count == 0 {
            "No browser cart items yet".to_string()
        } else {
            format!("{count} x Charizard Holo staged in the browser island")
        };
        let item_word = if count == 1 { "item" } else { "items" };

    } =>
        Container(Column(
            gap: tokens.spacing.s,
            children: [
                Text::new(line)
                    .size(typography.body_large_size)
                    .line_height(typography.body_large_size * typography.line_height_normal)
                    .weight(typography.font_weight_bold)
                    .color(tokens.colors.text_secondary)
                    .semantics_identifier("island-cart-line"),
                Text::new(format!("{count} {item_word} in the browser island cart"))
                    .size(typography.body_medium_size)
                    .line_height(typography.body_medium_size * typography.line_height_normal)
                    .color(tokens.colors.text_muted)
                    .semantics_identifier("island-cart-count"),
            ],
        ))
        .padding_all(tokens.spacing.m)
        .border(tokens.colors.warning.with_alpha(90), 1.0)
        .border_radius(tokens.radii.large)
        .bg(tokens.colors.surface_raised)
}

fission_component! {
/// The subtotal and the button that adds another card.
    IslandCheckoutRow<BrowserCartState> { count: u32 }
    |row, _state, cx| {
        let tokens = &cx.view.env().theme.tokens;
        let typography = &tokens.typography;
        let subtotal = CHARIZARD_PRICE * row.count as f32;

    } =>
        Row(
            gap: tokens.spacing.m,
            align_items: ir_op::AlignItems::Center,
            children: [
                Column(
                    gap: tokens.spacing.xs,
                    children: [
                        Text::new("Island subtotal")
                            .size(typography.font_size_sm)
                            .line_height(typography.font_size_sm * typography.line_height_normal)
                            .weight(typography.font_weight_bold)
                            .color(tokens.colors.text_muted),
                        Text::new(format!("£{subtotal:.2}"))
                            .size(typography.heading_size)
                            .line_height(typography.heading_size * typography.line_height_heading)
                            .weight(typography.font_weight_bold)
                            .color(tokens.colors.text_primary)
                            .semantics_identifier("island-cart-total"),
                    ],
                ),
                Spacer(flex_grow: 1.0),
                SemanticsRegion(
                    Container(
                        Text::new("Add Charizard")
                            .size(typography.body_medium_size)
                            .line_height(
                                typography.body_medium_size * typography.line_height_normal
                            )
                            .weight(typography.font_weight_bold)
                            .color(tokens.colors.surface),
                    )
                    .padding([
                        tokens.spacing.m,
                        tokens.spacing.m,
                        tokens.spacing.s,
                        tokens.spacing.s,
                    ])
                    .border_radius(tokens.radii.full)
                    .bg(tokens.colors.warning),
                )
                .identifier("island-action:add-card")
                .role(fission::Role::Button)
                .default_action(cx.build.bind(IslandAddToCart, reduce_with!(on_island_add_to_cart)))
                .id(fission::WidgetId::explicit("island-action:add-card")),
            ],
        )
}

fission_component! {
/// Confirms the last update happened in the browser, without a page request.
    IslandLastEvent<BrowserCartState> { count: u32 }
    |event, _state, cx| {
        let typography = &cx.view.env().theme.tokens.typography;
        let text = if event.count == 0 {
            "Ready for client-side cart edits"
        } else {
            "Updated without a full page request"
        };
    } =>
        Text(text)
            .size(typography.font_size_sm)
            .line_height(typography.font_size_sm * typography.line_height_normal)
            .weight(typography.font_weight_semibold)
            .color(cx.view.env().theme.tokens.colors.warning)
            .semantics_identifier("island-last-event")
}

fission_component! {
/// A zero-size copy of the count, so browser tests can read the number without
/// parsing a sentence.
    IslandCountProbe<BrowserCartState> { count: u32 }
    |probe, _state, cx| { let tokens = &cx.view.env().theme.tokens; } =>
        Text(probe.count.to_string())
            .size(tokens.spacing.none)
            .line_height(tokens.spacing.none)
            .color(tokens.colors.surface_raised)
            .semantics_identifier("island-cart-count-short")
}

#[fission_reducer(IslandAddToCart)]
pub fn on_island_add_to_cart(state: &mut BrowserCartState) {
    state.count += 1;
}

pub fn cart_drawer_boot(input: &str) -> String {
    run_browser_island("cart-drawer", input, || {
        BrowserIslandApp::new(
            "cart-drawer",
            "cart-drawer",
            BrowserCartState::default(),
            CartDrawerIsland::new(),
        )
        .theme(fission::theme::Theme::dark())
    })
}
