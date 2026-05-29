use fission::server::{
    BrowserArtifactBuild, BrowserArtifactBuildOptions, ServerActionSigner, ServerRenderer,
    ServerRequest,
};
use pokemon_card_store::{app::AddToCart, cart::cart_service, pokemon_card_store_server};
use std::time::Duration;

fn cookie_header(response: &fission::server::ServerResponse) -> String {
    response
        .headers
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case("set-cookie"))
        .map(|(_, value)| value.split(';').next().unwrap_or(value).to_string())
        .expect("response should set a session cookie")
}

fn get_with_cookie(path: &str, cookie: &str) -> ServerRequest {
    let mut request = ServerRequest::get(path);
    request
        .headers
        .insert("cookie".to_string(), cookie.to_string());
    request
}

#[test]
fn store_home_renders_real_product_html_after_draining_catalog_job() {
    cart_service().clear_all();
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
    assert!(html.contains("Browser bridge"));
    assert!(html.contains("Island booting"));
    assert!(html.contains("worker-filter-summary"));
    assert!(html.contains("method=\"post\""));
    assert!(html.contains("name=\"token\""));
    assert!(response
        .headers
        .iter()
        .any(|(name, value)| name.eq_ignore_ascii_case("set-cookie")
            && value.contains("fission_session=")));
}

#[test]
fn signed_action_dispatches_reducer_before_rendering_response() {
    cart_service().clear_all();
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
    cart_service().clear_all();
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
    assert_eq!(response.status, 303);
    assert!(response
        .headers
        .iter()
        .any(|(name, value)| name.eq_ignore_ascii_case("location") && value == "/"));

    let cookie = cookie_header(&response);
    let redirected = renderer.handle(get_with_cookie("/", &cookie)).unwrap();
    let html = redirected.body_string();
    assert!(html.contains("1 item in the server cart"));
    assert!(html.contains("Last added: Pikachu Yellow Cheeks"));
}

#[test]
fn card_detail_route_renders_a_session_aware_product_page() {
    cart_service().clear_all();
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
fn cart_state_persists_across_requests_for_the_same_session() {
    cart_service().clear_all();
    let renderer = ServerRenderer::new(pokemon_card_store_server());

    let first = renderer.handle(ServerRequest::get("/")).unwrap();
    assert_eq!(first.status, 200);
    let cookie = cookie_header(&first);

    let token = renderer.sign_action(
        "/",
        0,
        AddToCart("charizard-holo".to_string()),
        Duration::from_secs(60),
    );
    let body = serde_json::to_vec(&token).unwrap();
    let mut action = ServerRequest::post("/__fission/action", body);
    action.headers.insert("cookie".to_string(), cookie.clone());
    let response = renderer.handle(action).unwrap();
    assert!(response.body_string().contains("1 item in the server cart"));

    let second = renderer.handle(get_with_cookie("/", &cookie)).unwrap();
    let html = second.body_string();
    assert!(html.contains("1 item in the server cart"));
    assert!(html.contains("Last added: Charizard Holo"));

    let other_session = renderer.handle(ServerRequest::get("/")).unwrap();
    assert!(other_session
        .body_string()
        .contains("0 items in the server cart"));
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
    assert!(island_source.contains("fission_bridge_alloc"));
    assert!(island_source.contains("fission_island_entry"));
    assert!(island_source.contains("fission_island_event"));

    let _ = std::fs::remove_dir_all(&root);
}
