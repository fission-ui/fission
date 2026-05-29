use crate::app::{StoreHomePage, StoreState};
use fission::server::{
    FissionServerApp, ProgressiveWorker, RevalidationPolicy, WasmIsland, WebRouteMode,
};
use std::time::Duration;

pub fn pokemon_card_store_server() -> FissionServerApp {
    FissionServerApp::new("Pokemon Card Store")
        .route_widget::<StoreState, _>(
            "/",
            "Pokemon Card Store",
            Some("A Fission server-rendered storefront for collectible cards.".to_string()),
            WebRouteMode::Revalidated(
                RevalidationPolicy::new(Duration::from_secs(300))
                    .stale_while_revalidate(Duration::from_secs(60))
                    .tags(["catalog", "pokemon-cards"]),
            ),
            StoreHomePage,
        )
        .worker(
            "/",
            ProgressiveWorker::new("catalog-filters", "/assets/workers/catalog-filters.wasm")
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
            .description("Focused Fission island for cart state, checkout totals, and item edits."),
        )
}
