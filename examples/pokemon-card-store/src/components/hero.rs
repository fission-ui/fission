use crate::app::StoreState;
use crate::data;
use fission::prelude::*;

#[derive(Clone)]
pub struct Hero;

impl Widget<StoreState> for Hero {
    fn build(&self, _ctx: &mut BuildCtx<StoreState>, _view: &View<StoreState>) -> Node {
        Container::new(
            Row {
                gap: Some(34.0),
                align_items: ir_op::AlignItems::Stretch,
                children: vec![copy(), spotlight()],
                ..Default::default()
            }
            .into_node(),
        )
        .padding_all(34.0)
        .border(color(59, 130, 246).with_alpha(80), 1.0)
        .border_radius(32.0)
        .bg(color(17, 24, 39))
        .into_node()
    }
}

fn copy() -> Node {
    Container::new(
        Column {
            gap: Some(18.0),
            children: vec![
                Text::new("Server-rendered collector commerce")
                    .size(58.0)
                    .line_height(62.0)
                    .weight(900)
                    .color(color(248, 250, 252))
                    .into_node(),
                Text::new("A Fission web store selling Pokémon cards with cacheable product pages, route-local enhancement workers, and a cart island that can load only when the shopper needs it.")
                    .size(18.0)
                    .line_height(29.0)
                    .color(color(203, 213, 225))
                    .into_node(),
                Row {
                    gap: Some(12.0),
                    children: vec![metric("6", "cards"), metric("5 min", "page TTL"), metric("2", "browser artifacts")],
                    ..Default::default()
                }
                .into_node(),
            ],
            ..Default::default()
        }
        .into_node(),
    )
    .flex_grow(1.0)
    .into_node()
}

fn spotlight() -> Node {
    let card = &data::cards()[0];
    Container::new(
        Column {
            gap: Some(16.0),
            children: vec![
                Text::new("Featured card")
                    .size(13.0)
                    .line_height(18.0)
                    .weight(800)
                    .color(color(251, 146, 60))
                    .into_node(),
                Text::new(card.name)
                    .size(34.0)
                    .line_height(40.0)
                    .weight(900)
                    .color(color(255, 247, 237))
                    .into_node(),
                Text::new(card.description)
                    .size(15.0)
                    .line_height(24.0)
                    .color(color(254, 215, 170))
                    .into_node(),
                Text::new(format!("£{:.2} · {} left", card.price, card.stock))
                    .size(18.0)
                    .line_height(24.0)
                    .weight(800)
                    .color(color(255, 237, 213))
                    .into_node(),
            ],
            ..Default::default()
        }
        .into_node(),
    )
    .width(360.0)
    .padding_all(28.0)
    .border(color(251, 146, 60).with_alpha(150), 1.0)
    .border_radius(28.0)
    .bg(color(card.accent.0, card.accent.1, card.accent.2).with_alpha(70))
    .into_node()
}

fn metric(value: &str, label: &str) -> Node {
    Container::new(
        Column {
            gap: Some(2.0),
            children: vec![
                Text::new(value)
                    .size(22.0)
                    .line_height(28.0)
                    .weight(900)
                    .color(color(248, 250, 252))
                    .into_node(),
                Text::new(label)
                    .size(12.0)
                    .line_height(16.0)
                    .color(color(148, 163, 184))
                    .into_node(),
            ],
            ..Default::default()
        }
        .into_node(),
    )
    .padding_all(14.0)
    .border(color(71, 85, 105), 1.0)
    .border_radius(18.0)
    .bg(color(15, 23, 42))
    .into_node()
}

fn color(r: u8, g: u8, b: u8) -> Color {
    Color { r, g, b, a: 255 }
}
