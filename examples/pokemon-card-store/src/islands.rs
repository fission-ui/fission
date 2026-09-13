use fission::prelude::*;
use fission::site::{run_browser_island, BrowserIslandApp};

#[derive(Debug, Default, Clone)]
pub struct BrowserCartState {
    count: u32,
}

impl GlobalState for BrowserCartState {}

/// What one Charizard Holo costs in the island's cart.
const CHARIZARD_PRICE: f32 = 249.00;

#[derive(Clone)]
pub struct CartDrawerIsland;

impl From<CartDrawerIsland> for Widget {
    fn from(_component: CartDrawerIsland) -> Self {
        let (_, view) = fission::build::current::<BrowserCartState>();
        let tokens = &view.env().theme.tokens;
        let count = view.state().count;

        Container::new(Column {
            gap: Some(tokens.spacing.m),
            children: widgets![
                IslandStatus { count },
                IslandCartSummary { count },
                IslandCheckoutRow { count },
                IslandLastEvent { count },
                IslandCountProbe { count },
            ],
            ..Default::default()
        })
        .into()
    }
}

/// Whether the island has handled a reducer event yet.
struct IslandStatus {
    count: u32,
}

impl From<IslandStatus> for Widget {
    fn from(status: IslandStatus) -> Self {
        let (_, view) = fission::build::current::<BrowserCartState>();
        let typography = &view.env().theme.tokens.typography;
        let colors = &view.env().theme.tokens.colors;
        let text = if status.count == 0 {
            "Island bridge ready"
        } else {
            "Island handled browser-side reducer event"
        };
        Text::new(text)
            .size(typography.body_medium_size)
            .line_height(typography.body_medium_size * typography.line_height_normal)
            .weight(typography.font_weight_bold)
            .color(colors.warning)
            .semantics_identifier("island-status:cart-drawer")
            .into()
    }
}

/// The staged line and item count, in a card outlined in the accent colour.
struct IslandCartSummary {
    count: u32,
}

impl From<IslandCartSummary> for Widget {
    fn from(summary: IslandCartSummary) -> Self {
        let (_, view) = fission::build::current::<BrowserCartState>();
        let tokens = &view.env().theme.tokens;
        let typography = &tokens.typography;
        let count = summary.count;
        let line = if count == 0 {
            "No browser cart items yet".to_string()
        } else {
            format!("{count} x Charizard Holo staged in the browser island")
        };
        let item_word = if count == 1 { "item" } else { "items" };

        Container::new(Column {
            gap: Some(tokens.spacing.s),
            children: widgets![
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
            ..Default::default()
        })
        .padding_all(tokens.spacing.m)
        .border(tokens.colors.warning.with_alpha(90), 1.0)
        .border_radius(tokens.radii.large)
        .bg(tokens.colors.surface_raised)
        .into()
    }
}

/// The subtotal and the button that adds another card.
struct IslandCheckoutRow {
    count: u32,
}

impl From<IslandCheckoutRow> for Widget {
    fn from(row: IslandCheckoutRow) -> Self {
        let (ctx, view) = fission::build::current::<BrowserCartState>();
        let tokens = &view.env().theme.tokens;
        let typography = &tokens.typography;
        let subtotal = CHARIZARD_PRICE * row.count as f32;

        Row {
            gap: Some(tokens.spacing.m),
            align_items: ir_op::AlignItems::Center,
            children: widgets![
                Column {
                    gap: Some(tokens.spacing.xs),
                    children: widgets![
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
                    ..Default::default()
                },
                Spacer {
                    flex_grow: 1.0,
                    ..Default::default()
                },
                SemanticsRegion::new(
                    Container::new(
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
                .default_action(ctx.bind(IslandAddToCart, reduce_with!(on_island_add_to_cart)))
                .id(fission::WidgetId::explicit("island-action:add-card")),
            ],
            ..Default::default()
        }
        .into()
    }
}

/// Confirms the last update happened in the browser, without a page request.
struct IslandLastEvent {
    count: u32,
}

impl From<IslandLastEvent> for Widget {
    fn from(event: IslandLastEvent) -> Self {
        let (_, view) = fission::build::current::<BrowserCartState>();
        let typography = &view.env().theme.tokens.typography;
        let text = if event.count == 0 {
            "Ready for client-side cart edits"
        } else {
            "Updated without a full page request"
        };
        Text::new(text)
            .size(typography.font_size_sm)
            .line_height(typography.font_size_sm * typography.line_height_normal)
            .weight(typography.font_weight_semibold)
            .color(view.env().theme.tokens.colors.warning)
            .semantics_identifier("island-last-event")
            .into()
    }
}

/// A zero-size copy of the count, so browser tests can read the number without
/// parsing a sentence.
struct IslandCountProbe {
    count: u32,
}

impl From<IslandCountProbe> for Widget {
    fn from(probe: IslandCountProbe) -> Self {
        let (_, view) = fission::build::current::<BrowserCartState>();
        let tokens = &view.env().theme.tokens;
        Text::new(probe.count.to_string())
            .size(tokens.spacing.none)
            .line_height(tokens.spacing.none)
            .color(tokens.colors.surface_raised)
            .semantics_identifier("island-cart-count-short")
            .into()
    }
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
            CartDrawerIsland,
        )
        .theme(fission::theme::Theme::dark())
    })
}
