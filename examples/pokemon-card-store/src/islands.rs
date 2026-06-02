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
            gap: Some(14.0),
            children: vec![
                Text::new(status)
                    .size(13.0)
                    .line_height(18.0)
                    .weight(800)
                    .color(color(251, 191, 36))
                    .semantics_identifier("island-status:cart-drawer")
                    .into(),
                Container::new(Column {
                    gap: Some(6.0),
                    children: vec![
                        Text::new(line)
                            .size(15.0)
                            .line_height(21.0)
                            .weight(800)
                            .color(color(226, 232, 240))
                            .semantics_identifier("island-cart-line")
                            .into(),
                        Text::new(format!("{count} {item_word} in the browser island cart"))
                            .size(13.0)
                            .line_height(18.0)
                            .color(color(148, 163, 184))
                            .semantics_identifier("island-cart-count")
                            .into(),
                    ],
                    ..Default::default()
                })
                .padding_all(14.0)
                .border(color(251, 191, 36).with_alpha(90), 1.0)
                .border_radius(16.0)
                .bg(color(30, 41, 59))
                .into(),
                Row {
                    gap: Some(12.0),
                    children: vec![
                        Column {
                            gap: Some(4.0),
                            children: vec![
                                Text::new("Island subtotal")
                                    .size(12.0)
                                    .line_height(16.0)
                                    .weight(800)
                                    .color(color(148, 163, 184))
                                    .into(),
                                Text::new(format!("£{subtotal:.2}"))
                                    .size(24.0)
                                    .line_height(30.0)
                                    .weight(900)
                                    .color(color(248, 250, 252))
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
                                    .size(14.0)
                                    .line_height(20.0)
                                    .weight(900)
                                    .color(color(15, 23, 42)),
                            )
                            .padding([14.0, 14.0, 12.0, 12.0])
                            .border_radius(999.0)
                            .bg(color(251, 191, 36)),
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
                .size(12.0)
                .line_height(17.0)
                .weight(700)
                .color(color(251, 191, 36))
                .semantics_identifier("island-last-event")
                .into(),
                Text::new(count.to_string())
                    .size(1.0)
                    .line_height(1.0)
                    .color(color(24, 35, 58))
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

fn color(r: u8, g: u8, b: u8) -> Color {
    Color { r, g, b, a: 255 }
}
