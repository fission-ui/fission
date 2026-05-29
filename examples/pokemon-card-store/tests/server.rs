use fission::server::{
    BrowserArtifactBuild, BrowserArtifactBuildOptions, Cache, CacheKey, Freshness, MokaCache,
    ServerActionSigner, ServerRenderer, ServerRequest,
};
use pokemon_card_store::{app::AddToCart, pokemon_card_store_server};
use std::sync::Arc;
use std::time::{Duration, SystemTime};

#[test]
fn store_home_renders_real_product_html_after_draining_catalog_job() {
    let renderer = ServerRenderer::new(pokemon_card_store_server());
    let response = renderer.handle(ServerRequest::get("/")).unwrap();
    let html = response.body_string();

    assert_eq!(response.status, 200);
    assert!(html.contains("Charizard Holo"));
    assert!(html.contains("Fission Card Market"));
    assert!(html.contains("0 items in the server cart"));
    assert!(html.contains("fission-route-manifest"));
    assert!(html.contains("catalog-filters"));
    assert!(html.contains("cart-drawer"));
    assert!(html.contains("View details"));
    assert!(html.contains("cards/charizard-holo/"));
    assert!(html.contains("method=\"post\""));
    assert!(html.contains("name=\"token\""));
}

#[test]
fn signed_action_dispatches_reducer_before_rendering_response() {
    let renderer = ServerRenderer::new(pokemon_card_store_server());
    let token = renderer.sign_action(
        "/",
        0,
        AddToCart("charizard-holo".to_string()),
        Duration::from_secs(60),
    );
    let body = serde_json::to_vec(&token).unwrap();
    let response = renderer
        .handle(ServerRequest::post("/__fission/action", body))
        .unwrap();
    let html = response.body_string();

    assert_eq!(response.status, 200);
    assert!(html.contains("1 item in the server cart"));
    assert!(html.contains("Charizard Holo"));
}

#[test]
fn form_encoded_action_token_dispatches_like_browser_submit() {
    let renderer = ServerRenderer::new(pokemon_card_store_server());
    let token = renderer.sign_action(
        "/",
        0,
        AddToCart("pikachu-yellow-cheeks".to_string()),
        Duration::from_secs(60),
    );
    let encoded = ServerActionSigner::development()
        .encode(&token)
        .expect("encode token");
    let mut request = ServerRequest::post("/__fission/action", format!("token={encoded}"));
    request.headers.insert(
        "content-type".to_string(),
        "application/x-www-form-urlencoded".to_string(),
    );

    let response = renderer.handle(request).unwrap();
    let html = response.body_string();
    assert_eq!(response.status, 200);
    assert!(html.contains("1 item in the server cart"));
    assert!(html.contains("Pikachu Yellow Cheeks"));
}

#[test]
fn card_detail_route_renders_a_cacheable_product_page() {
    let renderer = ServerRenderer::new(pokemon_card_store_server());
    let response = renderer
        .handle(ServerRequest::get("/cards/charizard-holo"))
        .unwrap();
    let html = response.body_string();

    assert_eq!(response.status, 200);
    assert!(html.contains("Card details"));
    assert!(html.contains("Add this card to basket"));
    assert!(html.contains("Charizard Holo"));
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

#[test]
fn browser_artifact_build_writes_worker_and_island_shims() {
    let root = std::env::temp_dir().join(format!(
        "pokemon-card-store-artifacts-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    let output_dir = root.join("target/fission/server");
    let options = BrowserArtifactBuildOptions {
        project_dir: std::env::current_dir().unwrap(),
        output_dir: output_dir.clone(),
        package_name: "pokemon-card-store".to_string(),
        package_default_features: false,
        package_features: vec!["browser".to_string()],
        release: false,
        compile: false,
    };

    let build = BrowserArtifactBuild::from_app(&pokemon_card_store_server(), &options).unwrap();
    assert_eq!(build.plans.len(), 2);
    build.write_shims(&options).unwrap();

    let worker_manifest =
        std::fs::read_to_string(output_dir.join("generated/workers/catalog-filters/Cargo.toml"))
            .unwrap();
    let island_source =
        std::fs::read_to_string(output_dir.join("generated/islands/cart-drawer/src/lib.rs"))
            .unwrap();
    assert!(worker_manifest.contains("package = \"pokemon-card-store\""));
    assert!(island_source.contains("pokemon_card_store::islands::cart_drawer_boot"));

    let _ = std::fs::remove_dir_all(&root);
}
