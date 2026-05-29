use crate::cart::{cart_service, CartService};
use crate::components::{card_grid::CardGrid, hero::Hero, shell::StoreShell};
use crate::data::{self, CatalogRequest, CatalogResponse, StoreError, CATALOG_JOB};
use fission::core::ResourceKey;
use fission::prelude::*;

#[derive(Debug, Clone)]
pub struct StoreState {
    pub catalog: AsyncSnapshot<CatalogResponse, StoreError>,
    pub session_id: String,
    pub cart_items: Vec<String>,
}

impl Default for StoreState {
    fn default() -> Self {
        Self {
            catalog: AsyncSnapshot::waiting(),
            session_id: String::new(),
            cart_items: Vec::new(),
        }
    }
}

impl AppState for StoreState {}

impl StoreState {
    pub fn for_session(session_id: impl Into<String>) -> Self {
        let session_id = session_id.into();
        Self {
            catalog: AsyncSnapshot::waiting(),
            cart_items: cart_service().load(&session_id).items,
            session_id,
        }
    }
}

#[derive(Clone)]
pub struct StoreHomePage;

impl Widget<StoreState> for StoreHomePage {
    fn build(&self, ctx: &mut BuildCtx<StoreState>, view: &View<StoreState>) -> Node {
        let catalog_loaded = with_reducer!(ctx, CatalogLoaded, on_catalog_loaded);
        let catalog_failed = with_reducer!(ctx, CatalogFailed, on_catalog_failed);
        ctx.register::<AddToCart, _>(reduce_with!(on_add_to_cart));

        let catalog_request = CatalogRequest { generation: 1 };
        let catalog_snapshot = view.state.catalog.clone();
        let card_grid = FutureBuilder::new(
            ResourceKey::new("pokemon-card-store.catalog"),
            CATALOG_JOB,
            catalog_request.clone(),
            catalog_snapshot,
            |ctx, view, snapshot| {
                CardGrid {
                    snapshot: snapshot.clone(),
                }
                .build(ctx, view)
            },
        )
        .deps(catalog_request)
        .on_ok(catalog_loaded)
        .on_err(catalog_failed)
        .build(ctx, view);

        StoreShell {
            child: Column {
                gap: Some(28.0),
                children: vec![
                    Hero.build(ctx, view),
                    cart_summary(view),
                    card_grid,
                    browser_runtime_panel(),
                ],
                ..Default::default()
            }
            .into_node(),
        }
        .build(ctx, view)
    }
}

#[derive(Clone)]
pub struct StoreCardPage {
    pub slug: String,
}

impl Widget<StoreState> for StoreCardPage {
    fn build(&self, ctx: &mut BuildCtx<StoreState>, view: &View<StoreState>) -> Node {
        let Some(card) = data::card_by_slug(&self.slug) else {
            return StoreShell {
                child: not_found(&self.slug),
            }
            .build(ctx, view);
        };
        let add = ctx.bind(
            AddToCart(card.slug.to_string()),
            reduce_with!(on_add_to_cart),
        );
        let accent = color(card.accent.0, card.accent.1, card.accent.2);
        StoreShell {
            child: Column {
                gap: Some(24.0),
                children: vec![
                    Text::new("Card details")
                        .size(14.0)
                        .line_height(18.0)
                        .weight(800)
                        .color(accent)
                        .semantics_identifier("site-route:/")
                        .into_node(),
                    Container::new(
                        Row {
                            gap: Some(28.0),
                            align_items: ir_op::AlignItems::Stretch,
                            children: vec![
                                detail_art(card),
                                Column {
                                    gap: Some(18.0),
                                    children: vec![
                                        Text::new(card.name)
                                            .size(48.0)
                                            .line_height(54.0)
                                            .weight(900)
                                            .color(color(248, 250, 252))
                                            .into_node(),
                                        Text::new(format!(
                                            "{} · {} · {}",
                                            card.set, card.rarity, card.type_line
                                        ))
                                        .size(16.0)
                                        .line_height(24.0)
                                        .weight(800)
                                        .color(accent)
                                        .into_node(),
                                        Text::new(card.description)
                                            .size(18.0)
                                            .line_height(30.0)
                                            .color(color(203, 213, 225))
                                            .into_node(),
                                        Text::new(format!(
                                            "£{:.2} · {} currently in stock",
                                            card.price, card.stock
                                        ))
                                        .size(24.0)
                                        .line_height(30.0)
                                        .weight(900)
                                        .color(color(255, 255, 255))
                                        .into_node(),
                                        Button {
                                            variant: ButtonVariant::Filled,
                                            child: Some(Box::new(
                                                Text::new("Add this card to basket").into_node(),
                                            )),
                                            on_press: Some(add),
                                            ..Default::default()
                                        }
                                        .into_node(),
                                        cart_summary(view),
                                    ],
                                    ..Default::default()
                                }
                                .into_node(),
                            ],
                            ..Default::default()
                        }
                        .into_node(),
                    )
                    .padding_all(28.0)
                    .border(accent.with_alpha(120), 1.0)
                    .border_radius(30.0)
                    .bg(color(15, 23, 42))
                    .into_node(),
                ],
                ..Default::default()
            }
            .into_node(),
        }
        .build(ctx, view)
    }
}

#[fission_reducer(CatalogLoaded)]
pub fn on_catalog_loaded(state: &mut StoreState, ctx: &mut ReducerContext<StoreState>) {
    if let Some(catalog) = ctx.input.job_ok(CATALOG_JOB) {
        state.catalog = AsyncSnapshot::with_data(AsyncConnectionState::Done, catalog);
    }
}

#[fission_reducer(CatalogFailed)]
pub fn on_catalog_failed(state: &mut StoreState, ctx: &mut ReducerContext<StoreState>) {
    let error = ctx
        .input
        .job_err(CATALOG_JOB)
        .unwrap_or_else(|| StoreError {
            message: ctx
                .input
                .job_error_message(CATALOG_JOB)
                .unwrap_or("Unable to load the catalogue")
                .to_string(),
        });
    state.catalog = AsyncSnapshot::with_error(AsyncConnectionState::Done, error);
}

#[fission_reducer(AddToCart)]
pub fn on_add_to_cart(state: &mut StoreState, slug: String) {
    if data::card_by_slug(&slug).is_some() {
        if state.session_id.is_empty() {
            state.cart_items.push(slug);
        } else {
            state.cart_items = cart_service().add_item(&state.session_id, &slug).items;
        }
    }
}

fn cart_summary(view: &View<StoreState>) -> Node {
    let cart_count = view.state.cart_items.len();
    let last = view
        .state
        .cart_items
        .last()
        .and_then(|slug| data::card_by_slug(slug))
        .map(|card| format!("Last added: {}", card.name))
        .unwrap_or_else(|| "Choose a card to exercise signed server actions.".to_string());
    Container::new(
        Row {
            gap: Some(12.0),
            align_items: ir_op::AlignItems::Center,
            children: vec![
                Text::new(format!(
                    "{} {} in the server cart",
                    cart_count,
                    if cart_count == 1 { "item" } else { "items" }
                ))
                .size(16.0)
                .line_height(22.0)
                .weight(800)
                .color(color(219, 234, 254))
                .into_node(),
                Spacer {
                    flex_grow: 1.0,
                    ..Default::default()
                }
                .into_node(),
                Text::new(last)
                    .size(13.0)
                    .line_height(18.0)
                    .color(color(147, 197, 253))
                    .into_node(),
            ],
            ..Default::default()
        }
        .into_node(),
    )
    .padding([14.0, 14.0, 12.0, 12.0])
    .border(color(59, 130, 246).with_alpha(120), 1.0)
    .border_radius(18.0)
    .bg(color(30, 64, 175).with_alpha(80))
    .into_node()
}

fn browser_runtime_panel() -> Node {
    Container::new(
        Column {
            gap: Some(18.0),
            children: vec![
                Row {
                    gap: Some(14.0),
                    children: vec![
                        status_chip(
                            "Worker",
                            "worker-status:catalog-filters",
                            "Waiting for worker",
                        ),
                        status_chip("Island", "island-status:cart-drawer", "Waiting for island"),
                    ],
                    ..Default::default()
                }
                .into_node(),
                Text::new("Browser bridge")
                    .size(20.0)
                    .line_height(26.0)
                    .weight(900)
                    .color(color(248, 250, 252))
                    .into_node(),
                Text::new("The page is server rendered first. The worker and island artifacts then load as small WASM modules and update only the semantic targets they own.")
                    .size(14.0)
                    .line_height(22.0)
                    .color(color(148, 163, 184))
                    .into_node(),
                Row {
                    gap: Some(18.0),
                    children: vec![
                        Column {
                            gap: Some(10.0),
                            children: vec![
                                Text::new("Worker enhancement status pending")
                                    .size(13.0)
                                    .line_height(18.0)
                                    .color(color(148, 163, 184))
                                    .semantics_identifier("worker-filter-summary")
                                    .into_node(),
                                Text::new("This side represents progressive enhancement. The browser worker runs off the main thread and reports when route-local catalogue behaviour is ready.")
                                    .size(14.0)
                                    .line_height(22.0)
                                    .color(color(203, 213, 225))
                                    .into_node(),
                            ],
                            ..Default::default()
                        }
                        .flex_grow(1.0)
                        .into_node(),
                        browser_cart_island(),
                    ],
                    align_items: ir_op::AlignItems::Stretch,
                    ..Default::default()
                }
                .into_node(),
            ],
            ..Default::default()
        }
        .into_node(),
    )
    .padding_all(18.0)
    .border(color(71, 85, 105), 1.0)
    .border_radius(20.0)
    .bg(color(15, 23, 42))
    .into_node()
}

fn browser_cart_island() -> Node {
    SemanticsRegion::new(
        Container::new(
            Column {
                gap: Some(14.0),
                children: vec![
                    Text::new("Island booting")
                        .size(13.0)
                        .line_height(18.0)
                        .weight(800)
                        .color(color(251, 191, 36))
                        .semantics_identifier("island-status:cart-drawer")
                        .into_node(),
                    Text::new("The focused Fission island replaces this fallback with its own widget tree after its WASM artifact loads.")
                        .size(14.0)
                        .line_height(22.0)
                        .color(color(203, 213, 225))
                        .into_node(),
                ],
                ..Default::default()
            }
            .into_node(),
        )
        .width(440.0)
        .padding_all(18.0)
        .border(color(251, 191, 36).with_alpha(130), 1.0)
        .border_radius(24.0)
        .bg(color(24, 35, 58))
        .into_node(),
    )
    .id(fission::ir::NodeId::explicit("cart-drawer"))
    .identifier("cart-drawer")
    .into_node()
}

fn status_chip(label: &str, identifier: &str, status: &str) -> Node {
    Column {
        gap: Some(4.0),
        children: vec![
            Text::new(label)
                .size(12.0)
                .line_height(16.0)
                .weight(800)
                .color(color(148, 163, 184))
                .into_node(),
            Text::new(status)
                .size(15.0)
                .line_height(20.0)
                .weight(800)
                .color(color(226, 232, 240))
                .semantics_identifier(identifier)
                .into_node(),
        ],
        ..Default::default()
    }
    .into_node()
}

fn detail_art(card: &data::Card) -> Node {
    let accent = color(card.accent.0, card.accent.1, card.accent.2);
    Container::new(
        Column {
            gap: Some(14.0),
            children: vec![
                Text::new(card.type_line)
                    .size(16.0)
                    .line_height(22.0)
                    .weight(900)
                    .color(color(15, 23, 42))
                    .into_node(),
                Spacer {
                    flex_grow: 1.0,
                    ..Default::default()
                }
                .into_node(),
                Text::new(card.name)
                    .size(34.0)
                    .line_height(39.0)
                    .weight(900)
                    .color(color(15, 23, 42))
                    .into_node(),
            ],
            ..Default::default()
        }
        .into_node(),
    )
    .width(360.0)
    .height(500.0)
    .padding_all(28.0)
    .border_radius(24.0)
    .bg(accent)
    .into_node()
}

fn not_found(slug: &str) -> Node {
    Container::new(
        Column {
            gap: Some(12.0),
            children: vec![
                Text::new("Card not found")
                    .size(34.0)
                    .line_height(40.0)
                    .weight(900)
                    .color(color(248, 250, 252))
                    .into_node(),
                Text::new(format!("No card route exists for `{slug}`."))
                    .size(16.0)
                    .line_height(24.0)
                    .color(color(203, 213, 225))
                    .into_node(),
                Text::new("Back to catalogue")
                    .size(15.0)
                    .line_height(20.0)
                    .weight(800)
                    .color(color(147, 197, 253))
                    .semantics_identifier("site-route:/")
                    .into_node(),
            ],
            ..Default::default()
        }
        .into_node(),
    )
    .padding_all(28.0)
    .border(color(248, 113, 113).with_alpha(120), 1.0)
    .border_radius(24.0)
    .bg(color(15, 23, 42))
    .into_node()
}

fn color(r: u8, g: u8, b: u8) -> Color {
    Color { r, g, b, a: 255 }
}
