use crate::app::{on_add_to_cart, AddToCart, StoreState};
use crate::data::{Card, CatalogResponse, StoreError};
use fission::prelude::*;

#[derive(Clone)]
pub struct CardGrid {
    pub snapshot: AsyncSnapshot<CatalogResponse, StoreError>,
}

impl Widget<StoreState> for CardGrid {
    fn build(
        &self,
        ctx: &mut BuildCtx<StoreState>,
        _view: &View<StoreState>,
    ) -> impl fission::IntoWidget<StoreState> {
        fission::core::view::internal_node_widget({
            let Some(catalog) = self.snapshot.data() else {
                return fission::core::view::internal_node_widget(loading_or_error(&self.snapshot));
            };
            let mut children = Vec::new();
            for (index, summary) in catalog.cards.iter().enumerate() {
                let Some(card) = crate::data::card_by_slug(&summary.slug) else {
                    continue;
                };
                let row = (index / 3 + 1) as i16;
                let col = (index % 3 + 1) as i16;
                children.push(
                    GridItem::new(card_tile(ctx, card))
                        .cell(row, col)
                        .into_node(),
                );
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
        })
    }
}

fn loading_or_error(snapshot: &AsyncSnapshot<CatalogResponse, StoreError>) -> Node {
    let (title, detail, accent) = if let Some(error) = snapshot.error() {
        (
            "Catalogue unavailable",
            error.message.as_str(),
            color(248, 113, 113),
        )
    } else {
        (
            "Loading cards",
            "The server route declares a catalogue job and renders the completed state after the job drains.",
            color(96, 165, 250),
        )
    };
    Container::<Node>::lowered(
        Column {
            gap: Some(10.0),
            children: vec![
                Text::new(title)
                    .size(24.0)
                    .line_height(30.0)
                    .weight(900)
                    .color(color(248, 250, 252))
                    .into_node(),
                Text::new(detail)
                    .size(15.0)
                    .line_height(24.0)
                    .color(color(203, 213, 225))
                    .into_node(),
            ],
            ..Default::default()
        }
        .into_node(),
    )
    .padding_all(24.0)
    .border(accent.with_alpha(120), 1.0)
    .border_radius(24.0)
    .bg(color(15, 23, 42))
    .into_node()
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
                    Text::new("Generated as normal Fission widgets, rendered to HTML by the server shell, and combined with session cart state.")
                        .size(15.0)
                        .line_height(24.0)
                        .color(color(148, 163, 184))
                        .into_node(),
                ],
                ..Default::default()
            }
            .into_node(),
            Spacer { flex_grow: 1.0, ..Default::default() }.into_node(),
            Container::<Node>::lowered(
                Text::new("Cart updates persist through the server session")
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

fn card_tile(ctx: &mut BuildCtx<StoreState>, card: &Card) -> Node {
    let accent = color(card.accent.0, card.accent.1, card.accent.2);
    let add = ctx.bind(
        AddToCart(card.slug.to_string()),
        reduce_with!(on_add_to_cart),
    );
    Container::<Node>::lowered(
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
                Row {
                    gap: Some(10.0),
                    children: vec![
                        Text::new("View details")
                            .size(14.0)
                            .line_height(20.0)
                            .weight(800)
                            .color(accent)
                            .semantics_identifier(format!("site-route:/cards/{}/", card.slug))
                            .into_node(),
                        Spacer {
                            flex_grow: 1.0,
                            ..Default::default()
                        }
                        .into_node(),
                        Button {
                            variant: ButtonVariant::Outline,
                            child: Some(Box::new(Text::new("Add").into_node())),
                            on_press: Some(add),
                            padding: Some([12.0, 12.0, 8.0, 8.0]),
                            ..Default::default()
                        }
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
    Container::<Node>::lowered(
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
