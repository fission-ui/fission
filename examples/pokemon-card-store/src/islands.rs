use crate::components::palette::{
    AMBER, SURFACE, SURFACE_RAISED, TEXT_BODY, TEXT_MUTED, TEXT_PRIMARY,
};
use fission::prelude::*;
use fission::site::{run_browser_island, BrowserIslandApp};

#[derive(Debug, Default, Clone)]
pub struct BrowserCartState {
    count: u32,
}

impl GlobalState for BrowserCartState {}

#[derive(Clone)]
pub struct CartDrawerIsland;

impl From<CartDrawerIsland> for Widget {
    fn from(_component: CartDrawerIsland) -> Self {
        let (ctx, view) = fission::build::current::<BrowserCartState>();
        let tokens = &view.env().theme.tokens;
        let add = ctx.bind(IslandAddToCart, reduce_with!(on_island_add_to_cart));
        let count = view.state().count;
        let item_word = if count == 1 { "item" } else { "items" };
        let subtotal = 249.00 * count as f32;
        let line = if count == 0 {
            "No browser cart items yet".to_string()
        } else {
            format!("{count} x Charizard Holo staged in the browser island")
        };
        let status = if count == 0 {
            "Island bridge ready"
        } else {
            "Island handled browser-side reducer event"
        };

        Container::new(Column {
            gap: Some(tokens.spacing.m),
            children: vec![
                Text::new(status)
                    .size(tokens.typography.body_medium_size)
                    .line_height(
                        tokens.typography.body_medium_size * tokens.typography.line_height_normal,
                    )
                    .weight(tokens.typography.font_weight_bold)
                    .color(AMBER)
                    .semantics_identifier("island-status:cart-drawer")
                    .into(),
                Container::new(Column {
                    gap: Some(tokens.spacing.s),
                    children: vec![
                        Text::new(line)
                            .size(tokens.typography.body_large_size)
                            .line_height(
                                tokens.typography.body_large_size
                                    * tokens.typography.line_height_normal,
                            )
                            .weight(tokens.typography.font_weight_bold)
                            .color(TEXT_BODY)
                            .semantics_identifier("island-cart-line")
                            .into(),
                        Text::new(format!("{count} {item_word} in the browser island cart"))
                            .size(tokens.typography.body_medium_size)
                            .line_height(
                                tokens.typography.body_medium_size
                                    * tokens.typography.line_height_normal,
                            )
                            .color(TEXT_MUTED)
                            .semantics_identifier("island-cart-count")
                            .into(),
                    ],
                    ..Default::default()
                })
                .padding_all(tokens.spacing.m)
                .border(AMBER.with_alpha(90), 1.0)
                .border_radius(tokens.radii.large)
                .bg(SURFACE_RAISED)
                .into(),
                Row {
                    gap: Some(tokens.spacing.m),
                    children: vec![
                        Column {
                            gap: Some(tokens.spacing.xs),
                            children: vec![
                                Text::new("Island subtotal")
                                    .size(tokens.typography.font_size_sm)
                                    .line_height(
                                        tokens.typography.font_size_sm
                                            * tokens.typography.line_height_normal,
                                    )
                                    .weight(tokens.typography.font_weight_bold)
                                    .color(TEXT_MUTED)
                                    .into(),
                                Text::new(format!("£{subtotal:.2}"))
                                    .size(tokens.typography.heading_size)
                                    .line_height(
                                        tokens.typography.heading_size
                                            * tokens.typography.line_height_heading,
                                    )
                                    .weight(tokens.typography.font_weight_bold)
                                    .color(TEXT_PRIMARY)
                                    .semantics_identifier("island-cart-total")
                                    .into(),
                            ],
                            ..Default::default()
                        }
                        .into(),
                        Spacer {
                            flex_grow: 1.0,
                            ..Default::default()
                        }
                        .into(),
                        SemanticsRegion::new(
                            Container::new(
                                Text::new("Add Charizard")
                                    .size(tokens.typography.body_medium_size)
                                    .line_height(
                                        tokens.typography.body_medium_size
                                            * tokens.typography.line_height_normal,
                                    )
                                    .weight(tokens.typography.font_weight_bold)
                                    .color(SURFACE),
                            )
                            .padding([
                                tokens.spacing.m,
                                tokens.spacing.m,
                                tokens.spacing.s,
                                tokens.spacing.s,
                            ])
                            .border_radius(tokens.radii.full)
                            .bg(AMBER),
                        )
                        .identifier("island-action:add-card")
                        .role(fission::Role::Button)
                        .default_action(add)
                        .id(fission::WidgetId::explicit("island-action:add-card"))
                        .into(),
                    ],
                    align_items: ir_op::AlignItems::Center,
                    ..Default::default()
                }
                .into(),
                Text::new(if count == 0 {
                    "Ready for client-side cart edits"
                } else {
                    "Updated without a full page request"
                })
                .size(tokens.typography.font_size_sm)
                .line_height(tokens.typography.font_size_sm * tokens.typography.line_height_normal)
                .weight(tokens.typography.font_weight_semibold)
                .color(AMBER)
                .semantics_identifier("island-last-event")
                .into(),
                Text::new(count.to_string())
                    .size(tokens.spacing.none)
                    .line_height(tokens.spacing.none)
                    .color(SURFACE_RAISED)
                    .semantics_identifier("island-cart-count-short")
                    .into(),
            ],
            ..Default::default()
        })
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
    })
}
