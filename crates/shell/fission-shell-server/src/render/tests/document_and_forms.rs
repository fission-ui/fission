use super::*;

#[test]
fn server_renderer_serves_site_css_and_enhancement_script() {
    let renderer = ServerRenderer::new(
        FissionServerApp::new("Test").server_route_widget::<TestState, _>(
            "/",
            "Home",
            None,
            TestPage("Asset page"),
        ),
    );

    let page = renderer.handle(ServerRequest::get("/")).unwrap();
    assert_eq!(page.status, 200);

    let css = renderer.handle(ServerRequest::get("/site.css")).unwrap();
    assert_eq!(css.status, 200);
    assert_eq!(
        response_header(&css, "content-type"),
        Some("text/css; charset=utf-8")
    );
    let css = css.body_string();
    assert!(css.contains(".fission-site-root"));
    assert!(css.contains(".fission-site-positioned > .fission-site-semantics"));
    assert!(css.contains("button.fission-site-semantics"));
    assert!(css.contains(".fission-site-retained-control-input"));
    assert!(css.contains(
        ".fission-browser-action>.fission-site-box,.fission-server-action>.fission-site-box{display:flex;align-items:center;}"
    ));
    assert!(css.contains(":root"));

    let js = renderer
        .handle(ServerRequest::get("/site-enhancement.js"))
        .unwrap();
    assert_eq!(js.status, 200);
    assert_eq!(
        response_header(&js, "content-type"),
        Some("application/javascript; charset=utf-8")
    );
    assert!(js.body_string().contains("fission-site-js"));

    let runtime = renderer
        .handle(ServerRequest::get("/server-runtime.js"))
        .unwrap();
    assert_eq!(runtime.status, 200);
    assert_eq!(
        response_header(&runtime, "content-type"),
        Some("application/javascript; charset=utf-8")
    );
    let runtime = runtime.body_string();
    assert!(runtime.contains("fission_bridge_alloc"));
    assert!(runtime.contains("fission-site-text-run"));
    assert!(runtime.contains("data-fission-browser-text-action"));
    assert!(runtime.contains("nativeCheckable?'change':'click'"));
    assert!(runtime.contains("selectionDirection"));
    assert!(runtime.contains("textEncoder.encode(value.slice(0,bounded)).length"));
    assert!(runtime.contains("compositionCommitValue"));
    assert!(runtime.contains("__FISSION_SERVER_RUNTIME_TEST_HOOK__"));
}

#[test]
fn server_runtime_dom_event_fixture_passes_when_node_is_available() {
    let node = match std::process::Command::new("node").arg("--version").output() {
        Ok(output) if output.status.success() => "node",
        Ok(_) | Err(_) => {
            eprintln!("node is unavailable; skipping server runtime DOM-event fixture");
            return;
        }
    };
    let fixture =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/server-runtime.test.mjs");
    let status = std::process::Command::new(node)
        .arg("--test")
        .arg(&fixture)
        .status()
        .expect("failed to launch server runtime DOM-event fixture");
    assert!(status.success(), "server runtime DOM-event fixture failed");
}

#[test]
fn server_renderer_emits_switchable_light_dark_theme_css() {
    let light = fission_theme::Theme::default();
    let dark = fission_theme::Theme::dark();
    let renderer = ServerRenderer::new(
        FissionServerApp::new("Test")
            .light_dark_themes(light.clone(), dark.clone(), DesignMode::Dark)
            .server_route_widget::<TestState, _>("/", "Home", None, TestPage("Theme page")),
    );

    let css = renderer
        .handle(ServerRequest::get("/site.css"))
        .unwrap()
        .body_string();

    assert!(css.contains(&theme_variables_css(":root,[data-theme=\"dark\"]", &dark)));
    assert!(css.contains(&theme_variables_css("[data-theme=\"light\"]", &light)));
    assert!(css.contains(&theme_variables_css("[data-theme=\"dark\"]", &dark)));
}

#[test]
fn server_renderer_enables_theme_switching_document_contract() {
    let renderer = ServerRenderer::new(
        FissionServerApp::new("Test")
            .light_dark_themes(
                fission_theme::Theme::default(),
                fission_theme::Theme::dark(),
                DesignMode::Dark,
            )
            .server_route_widget::<TestState, _>("/", "Home", None, TestPage("Theme page")),
    );

    let html = renderer
        .handle(ServerRequest::get("/"))
        .unwrap()
        .body_string();

    assert!(html.contains(r#"<html lang="en" data-theme="dark">"#));
    assert!(html.contains("var k='fission-site-theme'"));
    assert!(html.contains("[data-fission-theme-toggle]"));
}

#[test]
fn server_renderer_uses_the_theme_toggle_semantics_label() {
    let renderer = ServerRenderer::new(
        FissionServerApp::new("Test")
            .light_dark_themes(
                fission_theme::Theme::default(),
                fission_theme::Theme::dark(),
                DesignMode::Light,
            )
            .server_route_widget::<TestState, _>("/", "Home", None, ThemeTogglePage),
    );

    let html = renderer
        .handle(ServerRequest::get("/"))
        .unwrap()
        .body_string();

    assert!(html.contains(r#"aria-label="Cambiar el tema de color""#));
}

#[test]
fn server_renderer_appends_user_css_to_site_stylesheet() {
    let renderer = ServerRenderer::new(
        FissionServerApp::new("Test")
            .user_css(".demo-hook{animation:demo 1s linear infinite;}")
            .server_route_widget::<TestState, _>("/", "Home", None, TestPage("CSS page")),
    );

    let page = renderer.handle(ServerRequest::get("/")).unwrap();
    assert_eq!(page.status, 200);

    let css = renderer.handle(ServerRequest::get("/site.css")).unwrap();
    assert_eq!(css.status, 200);
    let css = css.body_string();
    assert!(css.contains(".fission-site-root"));
    assert!(css.contains(".demo-hook{animation:demo 1s linear infinite;}"));
}

#[test]
fn shared_site_css_does_not_accumulate_rendered_route_styles() {
    let renderer = ServerRenderer::new(
        FissionServerApp::new("Test")
            .server_route_widget::<TestState, _>("/", "Home", None, TestPage("Home"))
            .server_route_widget::<TestState, _>("/catalog", "Catalog", None, CodePage),
    );

    let before = renderer
        .handle(ServerRequest::get("/site.css"))
        .unwrap()
        .body;
    renderer.handle(ServerRequest::get("/")).unwrap();
    renderer.handle(ServerRequest::get("/catalog")).unwrap();
    let after = renderer
        .handle(ServerRequest::get("/site.css"))
        .unwrap()
        .body;

    assert_eq!(before, after);
}

#[test]
fn packaged_fonts_are_not_exposed_by_default() {
    let renderer = ServerRenderer::new(
        FissionServerApp::new("Test")
            .with_fonts(&TEST_FONT)
            .server_route_widget::<TestState, _>("/", "Home", None, TestPage("Home")),
    );

    let css = renderer
        .handle(ServerRequest::get("/site.css"))
        .unwrap()
        .body_string();
    assert!(!css.contains("@font-face"));

    let font = renderer
        .handle(ServerRequest::get(packaged_font_asset_path(&TEST_FONT[0])))
        .unwrap();
    assert_eq!(font.status, 404);
}

#[test]
fn server_renderer_applies_document_elements_with_static_site_filters() {
    let renderer = ServerRenderer::new(
        FissionServerApp::new("Test")
            .page_element(SitePageElement::new(
                SitePageElementPlacement::HeadStart,
                "<meta name=\"global-head-start\" content=\"yes\">",
            ))
            .page_element(
                SitePageElement::head("<meta name=\"home-head-end\" content=\"yes\">")
                    .only_route("/"),
            )
            .page_element(
                SitePageElement::new(
                    SitePageElementPlacement::BodyStart,
                    "<div data-home-body-start></div>",
                )
                .only_route("/"),
            )
            .page_element(
                SitePageElement::body_end("<script>window.catalogReady=true;</script>")
                    .only_route("/catalog"),
            )
            .server_route_widget::<TestState, _>("/", "Home", None, TestPage("Home page"))
            .server_route_widget::<TestState, _>(
                "/catalog",
                "Catalog",
                None,
                TestPage("Catalog page"),
            ),
    );

    let home = renderer
        .handle(ServerRequest::get("/"))
        .unwrap()
        .body_string();
    assert!(home.contains("global-head-start"));
    assert!(home.contains("home-head-end"));
    assert!(home.contains("data-home-body-start"));
    assert!(!home.contains("window.catalogReady"));

    let catalog = renderer
        .handle(ServerRequest::get("/catalog"))
        .unwrap()
        .body_string();
    assert!(catalog.contains("global-head-start"));
    assert!(!catalog.contains("home-head-end"));
    assert!(!catalog.contains("data-home-body-start"));
    assert!(catalog.contains("window.catalogReady"));
}

#[test]
fn server_renderer_matches_static_document_assets_and_footer() {
    let renderer = ServerRenderer::new(
        FissionServerApp::new("Test")
            .with_fonts(&TEST_FONT)
            .with_font_delivery(fission_shell_site::WebFontDelivery::External)
            .favicon("/assets/icon.svg")
            .code_highlighting(fission_shell_site::CodeHighlightingOptions {
                enabled: true,
                ..Default::default()
            })
            .footer_widget::<TestState, _>(TestPage("Shared footer"))
            .server_route_widget::<TestState, _>("/", "Home", None, CodePage),
    );

    let html = renderer
        .handle(ServerRequest::get("/"))
        .unwrap()
        .body_string();

    assert!(html.contains("Shared footer"));
    assert!(html.contains("rel=\"icon\" href=\"/assets/icon.svg\""));
    assert!(html.contains("highlight.js/11.11.1/highlight.min.js"));
    let rendered = renderer.render_route("/").unwrap();
    assert!(!rendered.css.contains("@font-face"));
    assert!(!rendered.css.contains("data:font/"));

    let shared_css = renderer.handle(ServerRequest::get("/site.css")).unwrap();
    assert_eq!(
        response_header(&shared_css, "cache-control"),
        Some("public, max-age=300")
    );
    let shared_css = shared_css.body_string();
    assert!(shared_css.contains("font-family:'Parity Sans'"));
    assert!(shared_css.contains("src:url('__fission/fonts/"));
    assert!(!shared_css.contains("data:font/"));

    let font_path = packaged_font_asset_path(&TEST_FONT[0]);
    let font = renderer.handle(ServerRequest::get(font_path)).unwrap();
    assert_eq!(font.status, 200);
    assert_eq!(font.body, TEST_FONT[0].data);
    assert_eq!(
        response_header(&font, "cache-control"),
        Some("public, max-age=31536000, immutable")
    );

    let route_css_path = rendered
        .html
        .split("href=\"")
        .filter_map(|rest| rest.split_once('\"').map(|(href, _)| href))
        .find(|href| href.starts_with("/__fission/styles/"))
        .expect("content-addressed route stylesheet");
    let route_css = renderer.handle(ServerRequest::get(route_css_path)).unwrap();
    assert_eq!(route_css.status, 200);
    assert_eq!(route_css.body, rendered.css.as_bytes());
    assert_eq!(
        response_header(&route_css, "cache-control"),
        Some("public, max-age=31536000, immutable")
    );
}

#[test]
fn server_renderer_does_not_404_default_favicon_request() {
    let renderer = ServerRenderer::new(
        FissionServerApp::new("Test").server_route_widget::<TestState, _>(
            "/",
            "Home",
            None,
            TestPage("Favicon page"),
        ),
    );

    let response = renderer.handle(ServerRequest::get("/favicon.ico")).unwrap();
    assert_eq!(response.status, 204);
    assert_eq!(response.body.len(), 0);
}

#[test]
fn server_renderer_serves_project_assets_without_path_traversal() {
    let root = temp_project_dir("server-renderer-assets");
    let asset_dir = root.join("target/fission/server/assets/workers");
    fs::create_dir_all(&asset_dir).unwrap();
    fs::write(asset_dir.join("filters.wasm"), b"\0asm").unwrap();
    fs::write(root.join("secret.txt"), b"secret").unwrap();
    let renderer = ServerRenderer::new(
        FissionServerApp::new("Test")
            .project_dir(&root)
            .server_route_widget::<TestState, _>("/", "Home", None, TestPage("Asset page")),
    );

    let asset = renderer
        .handle(ServerRequest::get("/assets/workers/filters.wasm"))
        .unwrap();
    assert_eq!(asset.status, 200);
    assert_eq!(
        response_header(&asset, "content-type"),
        Some("application/wasm")
    );
    assert_eq!(asset.body, b"\0asm");

    let traversal = renderer
        .handle(ServerRequest::get("/assets/../secret.txt"))
        .unwrap();
    assert_eq!(traversal.status, 400);

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn server_renderer_serves_static_app_mount_with_index_and_modules() {
    let root = temp_project_dir("server-renderer-static-app");
    let admin_dir = root.join("assets/admin/pkg");
    fs::create_dir_all(&admin_dir).unwrap();
    fs::write(
        root.join("assets/admin/index.html"),
        "<!doctype html><script type=\"module\" src=\"./bootstrap.mjs\"></script>",
    )
    .unwrap();
    fs::write(
        root.join("assets/admin/bootstrap.mjs"),
        "import './pkg/app.js';",
    )
    .unwrap();
    fs::write(
        root.join("assets/admin/pkg/app.js"),
        "export const app = true;",
    )
    .unwrap();
    fs::write(root.join("secret.txt"), b"secret").unwrap();

    let renderer = ServerRenderer::new(
        FissionServerApp::new("Test")
            .project_dir(&root)
            .static_app("/admin", "assets/admin", "index.html")
            .server_route_widget::<TestState, _>("/", "Home", None, TestPage("Mount page")),
    );

    let index = renderer.handle(ServerRequest::get("/admin/")).unwrap();
    assert_eq!(index.status, 200);
    assert_eq!(
        response_header(&index, "content-type"),
        Some("text/html; charset=utf-8")
    );
    assert!(index.body_string().contains("bootstrap.mjs"));

    let module = renderer
        .handle(ServerRequest::get("/admin/bootstrap.mjs"))
        .unwrap();
    assert_eq!(module.status, 200);
    assert_eq!(
        response_header(&module, "content-type"),
        Some("application/javascript; charset=utf-8")
    );

    let nested = renderer
        .handle(ServerRequest::get("/admin/pkg/app.js"))
        .unwrap();
    assert_eq!(nested.status, 200);
    assert_eq!(
        response_header(&nested, "content-type"),
        Some("application/javascript; charset=utf-8")
    );

    let spa_route = renderer
        .handle(ServerRequest::get("/admin/content/calendar"))
        .unwrap();
    assert_eq!(spa_route.status, 200);
    assert_eq!(spa_route.body, index.body);

    let missing_asset = renderer
        .handle(ServerRequest::get("/admin/pkg/missing.js"))
        .unwrap();
    assert_eq!(missing_asset.status, 404);

    let traversal = renderer
        .handle(ServerRequest::get("/admin/../secret.txt"))
        .unwrap();
    assert_eq!(traversal.status, 400);

    let public_route = renderer.handle(ServerRequest::get("/")).unwrap();
    assert_eq!(public_route.status, 200);

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn server_renderer_serves_artifacts_from_container_artifact_root() {
    let _guard = env_lock().lock().unwrap();
    let root = temp_project_dir("server-renderer-env-assets");
    let artifact_root = root.join("server-artifacts/assets/islands");
    fs::create_dir_all(&artifact_root).unwrap();
    fs::write(artifact_root.join("cart.wasm"), b"\0asm").unwrap();
    std::env::set_var("FISSION_SERVER_ARTIFACTS", root.join("server-artifacts"));
    let renderer = ServerRenderer::new(
        FissionServerApp::new("Test")
            .project_dir(&root)
            .server_route_widget::<TestState, _>("/", "Home", None, TestPage("Asset page")),
    );

    let asset = renderer
        .handle(ServerRequest::get("/assets/islands/cart.wasm"))
        .unwrap();

    std::env::remove_var("FISSION_SERVER_ARTIFACTS");
    assert_eq!(asset.status, 200);
    assert_eq!(asset.body, b"\0asm");
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn signed_action_post_rejects_invalid_body_signature_origin_size_and_replay() {
    let renderer = ServerRenderer::new(
        FissionServerApp::new("Test").server_route_widget::<TestState, _>(
            "/",
            "Home",
            None,
            TestPage("Action page"),
        ),
    )
    .with_allowed_action_origin("https://app.example");
    let token = renderer.sign_action("/", 0, TestAction, Duration::from_secs(60));
    let body = serde_json::to_vec(&token).unwrap();

    let invalid_body = renderer
        .handle(ServerRequest::post(
            "/__fission/action",
            b"not-json".to_vec(),
        ))
        .unwrap();
    assert_eq!(invalid_body.status, 400);

    let oversized = renderer
        .handle(ServerRequest::post(
            "/__fission/action",
            vec![b'x'; MAX_SERVER_ACTION_BODY_BYTES + 1],
        ))
        .unwrap();
    assert_eq!(oversized.status, 413);

    let mut wrong_origin = ServerRequest::post("/__fission/action", body.clone());
    wrong_origin
        .headers
        .insert("origin".to_string(), "https://evil.example".to_string());
    assert_eq!(renderer.handle(wrong_origin).unwrap().status, 403);

    let mut allowed = ServerRequest::post("/__fission/action", body.clone());
    allowed
        .headers
        .insert("origin".to_string(), "https://app.example".to_string());
    assert_eq!(renderer.handle(allowed).unwrap().status, 200);

    let mut replay = ServerRequest::post("/__fission/action", body.clone());
    replay
        .headers
        .insert("origin".to_string(), "https://app.example".to_string());
    assert_eq!(renderer.handle(replay).unwrap().status, 403);

    let other_signer = ServerActionSigner::new("other-secret");
    let forged = other_signer.sign("/", 0, TestAction, Duration::from_secs(60));
    let mut forged_request =
        ServerRequest::post("/__fission/action", serde_json::to_vec(&forged).unwrap());
    forged_request
        .headers
        .insert("origin".to_string(), "https://app.example".to_string());
    assert_eq!(renderer.handle(forged_request).unwrap().status, 403);
}

#[test]
fn form_encoded_server_actions_redirect_back_to_the_route() {
    let renderer = ServerRenderer::new(
        FissionServerApp::new("Test").server_route_widget::<TestState, _>(
            "/cart",
            "Cart",
            None,
            TestPage("Cart page"),
        ),
    );
    let token = renderer.sign_action("/cart", 0, TestAction, Duration::from_secs(60));
    let encoded = renderer.action_signer.encode(&token).unwrap();
    let mut request = ServerRequest::post("/__fission/action", format!("token={encoded}"));
    request.headers.insert(
        "content-type".to_string(),
        "application/x-www-form-urlencoded".to_string(),
    );

    let response = renderer.handle(request).unwrap();

    assert_eq!(response.status, 303);
    assert_eq!(response_header(&response, "location"), Some("/cart/"));
    assert_eq!(
        response_header(&response, "cache-control"),
        Some("no-store")
    );
}

#[test]
fn typed_server_form_binds_current_controls_and_enforces_security_contract() {
    SUBMITTED_SEARCHES
        .get_or_init(|| Mutex::new(Vec::new()))
        .lock()
        .unwrap()
        .clear();
    let schema = ServerFormSchema::new("search")
        .field(ServerFormField::text("query").required().max_length(64))
        .field(ServerFormField::boolean("archived"))
        .field(ServerFormField::repeated("tag"));
    let renderer = ServerRenderer::new(
        FissionServerApp::new("Test")
            .form_action::<SubmittedSearch, SubmittedSearch, _>(schema, |form| form)
            .server_route_widget::<FormActionState, _>("/search", "Search", None, FormActionPage),
    )
    .with_allowed_action_origin("https://app.example");

    let initial_html = renderer
        .handle(ServerRequest::get("/search"))
        .unwrap()
        .body_string();
    assert!(initial_html.contains("name=\"query\""));
    assert!(initial_html.contains("form=\"search\""));
    assert!(initial_html.contains("id=\"search\""));
    assert!(initial_html.contains("action=\"/search/\""));

    let duplicate_token = rendered_form_token(&renderer);
    let duplicate = encoded_form(&[
        ("token", duplicate_token.as_str()),
        ("query", "one"),
        ("query", "two"),
    ]);
    assert_eq!(form_request(&renderer, duplicate, None).status, 422);

    let oversized = format!(
        "token={}&query={}",
        rendered_form_token(&renderer),
        "x".repeat(MAX_SERVER_ACTION_BODY_BYTES)
    );
    assert_eq!(form_request(&renderer, oversized, None).status, 413);

    let malformed_token = rendered_form_token(&renderer);
    let malformed = format!("token={malformed_token}&query=%FF");
    assert_eq!(form_request(&renderer, malformed, None).status, 400);

    let valid_token = rendered_form_token(&renderer);
    let valid = encoded_form(&[
        ("token", valid_token.as_str()),
        ("query", "naïve 海"),
        ("archived", "on"),
        ("tag", "water"),
        ("tag", "physics"),
    ]);
    assert_eq!(
        form_request(&renderer, valid.clone(), Some("https://evil.example")).status,
        403
    );
    assert_eq!(
        form_request(&renderer, valid.clone(), Some("https://app.example")).status,
        303
    );
    assert_eq!(
        SUBMITTED_SEARCHES.get().unwrap().lock().unwrap().last(),
        Some(&SubmittedSearch {
            query: "naïve 海".into(),
            archived: true,
            tag: vec!["water".into(), "physics".into()],
        })
    );
    let completed_token = rendered_form_token(&renderer);
    let completed = encoded_form(&[
        ("token", completed_token.as_str()),
        ("query", "rendered feedback"),
    ]);
    let completed = route_form_request(&renderer, completed, Some("https://app.example"));
    assert_eq!(completed.status, 200);
    assert!(completed
        .body_string()
        .contains("Search submitted: rendered feedback"));
    assert_eq!(
        SUBMITTED_SEARCHES.get().unwrap().lock().unwrap().last(),
        Some(&SubmittedSearch {
            query: "rendered feedback".into(),
            archived: false,
            tag: Vec::new(),
        })
    );
    assert_eq!(
        form_request(&renderer, valid, Some("https://app.example")).status,
        403
    );

    let absent_boolean_token = rendered_form_token(&renderer);
    let absent_boolean =
        encoded_form(&[("token", absent_boolean_token.as_str()), ("query", "open")]);
    assert_eq!(
        form_request(&renderer, absent_boolean, Some("https://app.example")).status,
        303
    );
    assert!(
        !SUBMITTED_SEARCHES
            .get()
            .unwrap()
            .lock()
            .unwrap()
            .last()
            .unwrap()
            .archived
    );
}

#[test]
fn typed_server_form_restores_signed_query_and_merges_signed_action_context() {
    let schema =
        ServerFormSchema::new("consent").field(ServerFormField::text("display_name").required());
    let renderer = ServerRenderer::new(
        FissionServerApp::new("Test")
            .form_action_with_context::<ConsentFields, ContextualConsent, _>(
                schema,
                |mut context, fields| {
                    context.display_name = fields.display_name;
                    context
                },
            )
            .route_widget_with_state::<ContextFormState, _, _>(
                "/consent",
                "Consent",
                None,
                WebRouteMode::Server(Default::default()),
                ContextFormPage,
                |ctx| {
                    Ok(ContextFormState {
                        oauth_state: ctx.request.query.get("state").cloned().unwrap_or_default(),
                        submitted: None,
                    })
                },
            ),
    );
    let mut initial = ServerRequest::get("/consent");
    initial
        .query
        .insert("state".into(), "trusted-oauth-state".into());
    let html = renderer.handle(initial).unwrap().body_string();
    let token = html
        .split("name=\"token\" value=\"")
        .nth(1)
        .and_then(|value| value.split('"').next())
        .unwrap();
    let body = encoded_form(&[("token", token), ("display_name", "Alex Rivera")]);
    let mut submitted = ServerRequest::post("/consent", body);
    submitted.headers.insert(
        "content-type".into(),
        "application/x-www-form-urlencoded".into(),
    );
    submitted
        .query
        .insert("state".into(), "unsigned-attacker-value".into());

    let response = renderer.handle(submitted).unwrap();

    assert_eq!(response.status, 200);
    let html = response.body_string();
    assert!(html.contains("request-state:trusted-oauth-state"));
    assert!(html.contains("submitted:trusted-oauth-state:Alex Rivera"));
    assert!(!html.contains("unsigned-attacker-value"));
}

fn rendered_form_token(renderer: &ServerRenderer) -> String {
    let html = renderer
        .handle(ServerRequest::get("/search"))
        .unwrap()
        .body_string();
    html.split("name=\"token\" value=\"")
        .nth(1)
        .and_then(|value| value.split('"').next())
        .unwrap()
        .to_string()
}

fn encoded_form(fields: &[(&str, &str)]) -> String {
    form_urlencoded::Serializer::new(String::new())
        .extend_pairs(fields.iter().copied())
        .finish()
}

fn form_request(renderer: &ServerRenderer, body: String, origin: Option<&str>) -> ServerResponse {
    let mut request = ServerRequest::post("/__fission/action", body);
    request.headers.insert(
        "content-type".into(),
        "application/x-www-form-urlencoded".into(),
    );
    if let Some(origin) = origin {
        request.headers.insert("origin".into(), origin.into());
    }
    renderer.handle(request).unwrap()
}

fn route_form_request(
    renderer: &ServerRenderer,
    body: String,
    origin: Option<&str>,
) -> ServerResponse {
    let mut request = ServerRequest::post("/search", body);
    request.headers.insert(
        "content-type".into(),
        "application/x-www-form-urlencoded".into(),
    );
    if let Some(origin) = origin {
        request.headers.insert("origin".into(), origin.into());
    }
    renderer.handle(request).unwrap()
}
