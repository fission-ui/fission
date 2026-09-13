use crate::app::{StoreCardPage, StoreHomePage, StoreState};
use crate::data::{cards, catalog_response, CATALOG_JOB};
use fission::server::{
    FissionServerApp, ProgressiveWorker, ServerJobRegistry, ServerPrivatePolicy, WasmIsland,
    WebRouteMode,
};

pub fn pokemon_card_store_server() -> FissionServerApp {
    let mut app = FissionServerApp::new("Pokemon Card Store")
        // The storefront is designed for the dark theme; colours come from its tokens.
        .theme(fission::theme::Theme::dark())
        .jobs(
            ServerJobRegistry::new()
                .register_job(CATALOG_JOB, |_request, _ctx| Ok(catalog_response())),
        )
        .route_widget_with_state::<StoreState, _, _>(
            "/",
            "Pokemon Card Store",
            Some("A Fission server-rendered storefront for collectible cards.".to_string()),
            WebRouteMode::ServerPrivate(ServerPrivatePolicy::default()),
            StoreHomePage,
            |ctx| Ok(StoreState::for_session(ctx.session.id())),
        )
        .worker(
            "/",
            ProgressiveWorker::new("catalog-filters", "/assets/workers/catalog-filters.wasm")
                .entry("pokemon_card_store::workers::catalog_filters_boot")
                .root_node_id("catalog-grid")
                .description("Client-side filtering and sort controls over server-rendered cards."),
        )
        .island(
            "/",
            WasmIsland::new(
                "cart-drawer",
                "/assets/islands/cart-drawer.wasm",
                "cart-drawer",
            )
            .entry("pokemon_card_store::islands::cart_drawer_boot")
            .description("Focused Fission island for cart state, checkout totals, and item edits."),
        );
    for card in cards() {
        app = app.route_widget_with_state::<StoreState, _, _>(
            format!("/cards/{}", card.slug),
            format!("{} | Pokemon Card Store", card.name),
            Some(card.description.to_string()),
            WebRouteMode::ServerPrivate(ServerPrivatePolicy::default()),
            StoreCardPage {
                slug: card.slug.to_string(),
            },
            |ctx| Ok(StoreState::for_session(ctx.session.id())),
        );
    }
    app
}
