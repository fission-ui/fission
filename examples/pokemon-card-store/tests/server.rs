use fission::server::{Cache, CacheKey, Freshness, MokaCache, ServerRenderer, ServerRequest};
use pokemon_card_store::pokemon_card_store_server;
use std::sync::Arc;
use std::time::SystemTime;

#[test]
fn store_home_renders_real_product_html() {
    let renderer = ServerRenderer::new(pokemon_card_store_server());
    let response = renderer.handle(ServerRequest::get("/")).unwrap();
    let html = response.body_string();

    assert_eq!(response.status, 200);
    assert!(html.contains("Charizard Holo"));
    assert!(html.contains("Fission Card Market"));
    assert!(html.contains("fission-route-manifest"));
    assert!(html.contains("catalog-filters"));
    assert!(html.contains("cart-drawer"));
}

#[test]
fn store_home_uses_revalidation_cache() {
    let cache = Arc::new(MokaCache::default());
    let renderer = ServerRenderer::new(pokemon_card_store_server()).with_cache(cache.clone());

    let first = renderer.handle(ServerRequest::get("/")).unwrap();
    assert_eq!(first.status, 200);
    let second = renderer.handle(ServerRequest::get("/")).unwrap();
    assert_eq!(second.cache_status, Some(Freshness::Fresh));
    assert!(cache
        .contains_fresh(&CacheKey::new("page:/?"), SystemTime::now())
        .unwrap());
}
