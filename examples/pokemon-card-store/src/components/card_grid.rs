use crate::app::StoreState;
use crate::data::Card;
use fission::prelude::*;

#[derive(Clone)]
pub struct CardGrid {
    pub cards: Vec<Card>,
}

impl Widget<StoreState> for CardGrid {
    fn build(&self, _ctx: &mut BuildCtx<StoreState>, _view: &View<StoreState>) -> Node {
        let mut children = Vec::new();
        for (index, card) in self.cards.iter().enumerate() {
            let row = (index / 3 + 1) as i16;
            let col = (index % 3 + 1) as i16;
            children.push(GridItem::new(card_tile(card)).cell(row, col).into_node());
        }
        Column {
            gap: Some(18.0),
            children: vec![
                section_title(),
                Grid {
                    columns: vec![
                        ir_op::GridTrack::Fr(1.0),
                        ir_op::GridTrack::Fr(1.0),
                        ir_op::GridTrack::Fr(1.0),
                    ],
                    rows: vec![ir_op::GridTrack::Auto, ir_op::GridTrack::Auto],
                    column_gap: Some(18.0),
                    row_gap: Some(18.0),
                    children,
                    ..Default::default()
                }
                .into_node(),
            ],
            ..Default::default()
        }
        .into_node()
    }
}

fn section_title() -> Node {
    Row {
        gap: Some(18.0),
        children: vec![
            Column {
                gap: Some(4.0),
                children: vec![
                    Text::new("Available cards")
                        .size(30.0)
                        .line_height(36.0)
                        .weight(900)
                        .color(color(248, 250, 252))
                        .into_node(),
                    Text::new("Generated as normal Fission widgets, rendered to HTML by the server shell, and cached with catalog tags.")
                        .size(15.0)
                        .line_height(24.0)
                        .color(color(148, 163, 184))
                        .into_node(),
                ],
                ..Default::default()
            }
            .into_node(),
            Spacer { flex_grow: 1.0, ..Default::default() }.into_node(),
            Container::new(
                Text::new("Inventory updates invalidate catalog/product tags")
                    .size(13.0)
                    .line_height(18.0)
                    .weight(700)
                    .color(color(187, 247, 208))
                    .into_node(),
            )
            .padding([12.0, 12.0, 8.0, 8.0])
            .border(color(34, 197, 94).with_alpha(120), 1.0)
            .border_radius(999.0)
            .bg(color(20, 83, 45).with_alpha(120))
            .into_node(),
        ],
        align_items: ir_op::AlignItems::Center,
        ..Default::default()
    }
    .into_node()
}

fn card_tile(card: &Card) -> Node {
    let accent = color(card.accent.0, card.accent.1, card.accent.2);
    Container::new(
        Column {
            gap: Some(14.0),
            children: vec![
                card_art(card),
                Text::new(card.name)
                    .size(22.0)
                    .line_height(27.0)
                    .weight(900)
                    .color(color(248, 250, 252))
                    .into_node(),
                Text::new(format!("{} · {}", card.set, card.rarity))
                    .size(13.0)
                    .line_height(18.0)
                    .weight(700)
                    .color(accent)
                    .into_node(),
                Text::new(card.description)
                    .size(14.0)
                    .line_height(22.0)
                    .color(color(203, 213, 225))
                    .into_node(),
                Row {
                    gap: Some(10.0),
                    children: vec![
                        Text::new(format!("£{:.2}", card.price))
                            .size(20.0)
                            .line_height(26.0)
                            .weight(900)
                            .color(color(255, 255, 255))
                            .into_node(),
                        Spacer {
                            flex_grow: 1.0,
                            ..Default::default()
                        }
                        .into_node(),
                        Text::new(format!("{} left", card.stock))
                            .size(13.0)
                            .line_height(18.0)
                            .color(color(148, 163, 184))
                            .into_node(),
                    ],
                    align_items: ir_op::AlignItems::Center,
                    ..Default::default()
                }
                .into_node(),
            ],
            ..Default::default()
        }
        .into_node(),
    )
    .padding_all(18.0)
    .border(accent.with_alpha(90), 1.0)
    .border_radius(24.0)
    .bg(color(15, 23, 42))
    .into_node()
}

fn card_art(card: &Card) -> Node {
    let accent = color(card.accent.0, card.accent.1, card.accent.2);
    Container::new(
        Column {
            gap: Some(8.0),
            children: vec![
                Text::new(card.type_line)
                    .size(12.0)
                    .line_height(16.0)
                    .weight(800)
                    .color(color(15, 23, 42))
                    .into_node(),
                Spacer {
                    flex_grow: 1.0,
                    ..Default::default()
                }
                .into_node(),
                Text::new(card.name)
                    .size(19.0)
                    .line_height(24.0)
                    .weight(900)
                    .color(color(15, 23, 42))
                    .into_node(),
            ],
            ..Default::default()
        }
        .into_node(),
    )
    .height(168.0)
    .padding_all(18.0)
    .border_radius(18.0)
    .bg(accent)
    .into_node()
}

fn color(r: u8, g: u8, b: u8) -> Color {
    Color { r, g, b, a: 255 }
}
