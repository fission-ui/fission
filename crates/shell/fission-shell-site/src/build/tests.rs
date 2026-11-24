use super::*;
use fission_core::ui::{Text, TextContent, Video};
use fission_core::{GlobalState, WidgetId};
use fission_i18n::TranslationBundle;
use fission_theme::{PackagedFont, PackagedFontStyle};
use fission_widgets::ProtectedRoute;
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Default)]
struct TestState;
impl GlobalState for TestState {}

static TEST_FONT: [PackagedFont; 1] = [PackagedFont {
    family: "Static Test Sans",
    weight: 400,
    style: PackagedFontStyle::Normal,
    format: "woff2",
    data: b"static-font-bytes",
    axes: &[],
}];

#[derive(Debug)]
struct RouteState(String);
impl GlobalState for RouteState {}

#[derive(Clone)]
struct RouteStatePage;

impl From<RouteStatePage> for Widget {
    fn from(_: RouteStatePage) -> Self {
        let (_, view) = fission_core::build::current::<RouteState>();
        Text::new(view.state().0.clone()).into()
    }
}

#[derive(Clone)]
struct KeyPage(&'static str);

impl From<KeyPage> for Widget {
    fn from(component: KeyPage) -> Self {
        let (_ctx, _view) = fission_core::build::current::<TestState>();
        Text::new(TextContent::Key(component.0.to_string())).into()
    }
}

#[derive(Clone)]
struct VideoPage;

impl From<VideoPage> for Widget {
    fn from(_component: VideoPage) -> Self {
        let (_ctx, _view) = fission_core::build::current::<TestState>();
        Video::network("https://example.com/demo.mp4")
            .id(WidgetId::explicit("site-video"))
            .size(320.0, 180.0)
            .autoplay(true)
            .loop_playback(true)
            .into()
    }
}

#[derive(Clone)]
struct PortalPage;

impl From<PortalPage> for Widget {
    fn from(_: PortalPage) -> Self {
        let (ctx, _) = fission_core::build::current::<TestState>();
        ctx.register_portal(Text::new("Static portal overlay").into());
        Text::new("Static portal root").into()
    }
}

fn translated_env() -> Env {
    let mut env = Env::default();
    env.i18n.add_bundle(TranslationBundle {
        locale: "fr".into(),
        messages: HashMap::from([("page.title".to_string(), "Bonjour static".to_string())]),
    });
    env
}

#[test]
fn clean_output_dir_removes_stale_files_without_removing_directories() {
    let temp = std::env::temp_dir().join(format!(
        "fission-site-clean-output-test-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let nested = temp.join("old/route");
    fs::create_dir_all(&nested).unwrap();
    fs::write(temp.join("old-index.html"), "obsolete").unwrap();
    fs::write(nested.join("index.html"), "obsolete").unwrap();

    clean_output_dir(&temp).unwrap();

    assert!(!temp.join("old-index.html").exists());
    assert!(!nested.join("index.html").exists());
    assert!(nested.is_dir());
    let _ = fs::remove_dir_all(temp);
}

#[test]
fn site_enhancement_positions_spotlight_regions() {
    let script = site_enhancement_js();

    assert!(script.contains("function initSpotlights"));
    assert!(script.contains("data-fission-spotlight-anchor"));
    assert!(script.contains("ResizeObserver"));
    assert!(script.contains("initSpotlights(document)"));
}

#[test]
fn route_paths_are_derived_from_content_tree() {
    let root = PathBuf::from("content/docs");
    assert_eq!(
        route_path_from_file("/docs", &root, Path::new("content/docs/index.md")),
        "/docs"
    );
    assert_eq!(
        route_path_from_file("/docs", &root, Path::new("content/docs/guides/start.md")),
        "/docs/guides/start"
    );
    assert_eq!(
        route_path_from_slug("/reference", "/widgets/button"),
        "/reference/widgets/button/"
    );
}

#[test]
fn route_links_render_as_boxed_click_targets() {
    let css = site_base_css();
    assert!(css.contains(".fission-site-route-link {\n  cursor: pointer;"));
    assert!(css.contains(".fission-site-route-link > .fission-site-node"));
    assert!(css.contains(".fission-site-positioned > .fission-site-semantics"));
    assert!(css.contains(".fission-site-svg {\n  display: block;"));
    assert!(css.contains(".fission-site-svg svg {\n  display: block;"));
    assert!(css.contains("width: 100%;\n  height: 100%;"));
    assert!(css.contains(".fission-site-svg-colored svg *"));
    assert!(css.contains("fill: currentColor;"));
    assert!(css.contains(".fission-site-svg-colored svg [fill=\"none\"]"));
    assert!(css.contains(".fission-site-footer {\n  position: static;"));
    assert!(css.contains("flex: 0 0 auto;"));
    assert!(css.contains(".fission-site-doc-main {\n  max-height: none;"));
    assert!(css.contains("min-height: calc(100vh - 4.75rem);"));
    assert!(css.contains(".fission-site-doc-sidebar > .fission-site-column,"));
    assert!(css.contains("align-self: stretch;\n  min-height: 100%;"));
    assert!(!css.contains(".fission-site-doc-layout {\n  height:"));
    assert!(!css.contains(".fission-site-doc-main {\n  max-height: 100%;"));
    assert!(!css
        .contains(".fission-site-route-link,\n.fission-site-heading-link { display: contents; }"));
}

#[test]
fn content_site_build_writes_real_html() {
    let temp = std::env::temp_dir().join(format!(
        "fission-site-test-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(temp.join("content")).unwrap();
    fs::create_dir_all(temp.join("assets")).unwrap();
    fs::write(temp.join("assets/favicon.svg"), "<svg></svg>").unwrap();
    fs::write(
        temp.join("content/getting-started.md"),
        "---\ntitle: Getting started\ndescription: First page\n---\n# Getting started\n\nThis is rendered by Fission.\n\n<Tabs>\n<TabItem value=\"rust\" label=\"Rust\">\n\nRust tab body.\n\n</TabItem>\n<TabItem value=\"site\" label=\"Site\">\n\nSite tab body.\n\n</TabItem>\n</Tabs>\n\n```rust\nlet answer = 42;\n```",
    )
    .unwrap();
    let mut options = SiteBuildOptions::for_project(&temp, "Test site");
    options.base_url = Some("https://example.com/docs".to_string());
    options.default_locale = "en-GB".to_string();
    options.site_favicon = Some("/favicon.svg".to_string());
    options.asset_dirs.push(temp.join("assets"));
    options.generate_sitemap = true;
    options.generate_robots = true;
    options.code_highlighting.enabled = true;
    options.search.enabled = true;
    options.site_nav =
        vec![
            SiteNavLink::new("Product", "/content/getting-started/").with_children(vec![
                SiteNavLink::new("Resources", "/content/getting-started/").with_children(vec![
                    SiteNavLink::new("Documentation", "/content/getting-started/"),
                ]),
            ]),
        ];
    options.page_elements.push(
        SitePageElement::head("<script defer src=\"https://example.com/site.js\"></script>")
            .only_route("/content/getting-started/"),
    );
    options.page_elements.push(
        SitePageElement::body_end("<script>window.exampleReady=true;</script>")
            .route_prefix("/content/"),
    );
    let report = build_content_site(&options).unwrap();
    let output = temp.join("target/fission/site/content/getting-started/index.html");
    assert_eq!(report.routes.len(), 1);
    assert!(output.exists());
    let html = fs::read_to_string(output).unwrap();
    assert!(html.contains("This is rendered by"));
    assert!(html.contains("Fission."));
    assert!(!html.contains("style=\""));
    assert!(html.contains("rel=\"canonical\""));
    assert!(html.contains("rel=\"icon\" href=\"../../favicon.svg\" type=\"image/svg+xml\""));
    assert!(html.contains("property=\"og:locale\" content=\"en_GB\""));
    assert!(html.contains("application/ld+json"));
    assert!(html.contains("https://example.com/site.js"));
    assert!(html.contains("window.exampleReady=true"));
    assert!(html.contains("<pre class=\"fission-site-code-block\""));
    assert!(html.contains("class=\"language-rust\""));
    assert!(html.contains("class=\"language-fission-tabs-start\""));
    assert!(html.contains("Rust tab"));
    assert!(html.contains("Site tab"));
    assert!(html.contains("site-enhancement.js"));
    assert!(html.contains("highlight.js/11.11.1/highlight.min.js"));
    assert!(html.contains("fission-site-nav-item"));
    assert!(html.contains("fission-site-nav-menu"));
    assert!(html.contains("Resources"));
    assert!(html.contains("Documentation"));
    assert!(html.contains("aria-label=\"Search this site\""));
    let css = fs::read_to_string(temp.join("target/fission/site/site.css")).unwrap();
    assert!(css.contains(":root"));
    assert!(css.contains(".fs_"));
    assert!(css.contains(".fission-doc-tabs"));
    assert!(
        !css.contains("min-height:864px"),
        "the documentation template must not bake its render viewport into browser flow"
    );
    assert!(temp.join("target/fission/site/sitemap.xml").exists());
    assert!(temp.join("target/fission/site/robots.txt").exists());
    let search_script =
        fs::read_to_string(temp.join("target/fission/site/search/search.js")).unwrap();
    assert!(search_script.contains("placeholder=\"Search this site\""));
    assert!(!search_script.contains("Fission docs"));
    assert!(temp
        .join("target/fission/site/search/manifest.json")
        .exists());
    let docs = fs::read_to_string(temp.join("target/fission/site/search/docs.json")).unwrap();
    assert!(docs.contains("Getting started"));
    let _ = fs::remove_dir_all(temp);
}

#[test]
fn omitted_and_nonempty_content_routes_keep_existing_mounts() {
    let temp = std::env::temp_dir().join(format!(
        "fission-site-route-defaults-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(temp.join("content")).unwrap();
    fs::write(temp.join("content/start.md"), "# Start\n").unwrap();
    fs::write(temp.join("fission.toml"), "[site]\n").unwrap();
    let defaults = SiteBuildOptions::from_project_dir(&temp, "Test").unwrap();
    assert_eq!(defaults.content_routes.len(), 1);
    assert_eq!(defaults.content_routes[0].path, "/content");
    assert_eq!(defaults.content_routes[0].source, temp.join("content"));
    let site = FissionSite::new();
    assert_eq!(check_site(&defaults, &site).unwrap().routes.len(), 1);

    fs::write(
        temp.join("fission.toml"),
        "[site]\n[[site.routes]]\npath = \"/docs\"\nsource = \"content\"\n",
    )
    .unwrap();
    let explicit = SiteBuildOptions::from_project_dir(&temp, "Test").unwrap();
    assert_eq!(explicit.content_routes.len(), 1);
    assert_eq!(explicit.content_routes[0].path, "/docs/");
    assert_eq!(explicit.content_routes[0].source, temp.join("content"));
    assert_eq!(build_site(&explicit, &site).unwrap().routes.len(), 1);
    assert!(explicit.output_dir.join("docs/start/index.html").exists());
    assert!(!explicit.output_dir.join("content").exists());
    fs::remove_dir_all(temp).unwrap();
}

#[test]
fn web_font_delivery_is_loaded_from_project_configuration() {
    let temp = std::env::temp_dir().join(format!(
        "fission-site-font-config-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(&temp).unwrap();
    fs::write(
        temp.join("fission.toml"),
        "[web]\nfont_delivery = \"external\"\n[site]\nroutes = []\n",
    )
    .unwrap();

    let options = SiteBuildOptions::from_project_dir(&temp, "Test").unwrap();
    assert_eq!(
        options.font_delivery,
        Some(crate::WebFontDelivery::External)
    );
    let _ = fs::remove_dir_all(temp);
}

#[test]
fn explicit_empty_content_routes_build_custom_pages_without_content_directory() {
    let temp = std::env::temp_dir().join(format!(
        "fission-site-custom-only-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(&temp).unwrap();
    fs::write(
        temp.join("fission.toml"),
        "[app]\nname = \"test-site\"\n[site]\nroutes = []\n",
    )
    .unwrap();
    let options = SiteBuildOptions::from_project_dir(&temp, "Test").unwrap();
    let site = FissionSite::new()
        .route_widget::<TestState, _>("/", "Home", None, Text::new("Home page"))
        .route_widget::<TestState, _>("/about/", "About", None, Text::new("About page"));
    assert_eq!(list_site_routes(&options, &site).unwrap().len(), 2);
    assert_eq!(check_site(&options, &site).unwrap().routes.len(), 2);
    assert_eq!(build_site(&options, &site).unwrap().routes.len(), 2);
    assert!(
        fs::read_to_string(options.output_dir.join("about/index.html"))
            .unwrap()
            .contains("About page")
    );
    assert!(!temp.join("content").exists());
    fs::write(
        temp.join("fission.toml"),
        "[app]\nname = \"test-site\"\n[site]\n",
    )
    .unwrap();
    let defaults = SiteBuildOptions::from_project_dir(&temp, "Test").unwrap();
    assert_eq!(defaults.content_routes.len(), 1);
    assert!(check_site(&defaults, &site).is_err());
    fs::remove_dir_all(temp).unwrap();
}

#[test]
fn custom_site_routes_resolve_keyed_text_from_seeded_env() {
    let temp = std::env::temp_dir().join(format!(
        "fission-site-i18n-test-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(&temp).unwrap();
    fs::create_dir_all(temp.join("content")).unwrap();
    let mut options = SiteBuildOptions::for_project(&temp, "Test site");
    options.content_routes = Vec::new();
    options.default_locale = "fr".to_string();
    let site = FissionSite::new()
        .with_env(translated_env())
        .route_widget::<TestState, _>("/", "Home", None, KeyPage("page.title"));

    let report = build_site(&options, &site).unwrap();
    let html = fs::read_to_string(temp.join("target/fission/site/index.html")).unwrap();

    assert_eq!(report.routes.len(), 1);
    assert!(html.contains("Bonjour static"));
    assert!(html.contains("lang=\"fr\""));
    assert!(!html.contains("MISSING:page.title"));
    let _ = fs::remove_dir_all(temp);
}

#[test]
fn static_site_omits_packaged_fonts_by_default() {
    let temp = std::env::temp_dir().join(format!(
        "fission-site-font-none-test-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(temp.join("content")).unwrap();
    let mut options = SiteBuildOptions::for_project(&temp, "Test site");
    options.content_routes = Vec::new();
    let site = FissionSite::new()
        .with_fonts(&TEST_FONT)
        .route_widget::<TestState, _>("/", "Home", None, Text::new("Home"));

    build_site(&options, &site).unwrap();

    let output = temp.join("target/fission/site");
    let css = fs::read_to_string(output.join("site.css")).unwrap();
    let relative = packaged_font_asset_href(&TEST_FONT[0]);
    assert!(!css.contains("@font-face"));
    assert!(!output.join(relative).exists());
    let _ = fs::remove_dir_all(temp);
}

#[test]
fn static_site_writes_external_font_assets_when_enabled() {
    let temp = std::env::temp_dir().join(format!(
        "fission-site-font-assets-test-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(temp.join("content")).unwrap();
    let mut options = SiteBuildOptions::for_project(&temp, "Test site");
    options.content_routes = Vec::new();
    let site = FissionSite::new()
        .with_fonts(&TEST_FONT)
        .with_font_delivery(crate::WebFontDelivery::External)
        .route_widget::<TestState, _>("/", "Home", None, Text::new("Home"));

    build_site(&options, &site).unwrap();

    let output = temp.join("target/fission/site");
    let css = fs::read_to_string(output.join("site.css")).unwrap();
    let relative = packaged_font_asset_href(&TEST_FONT[0]);
    assert!(css.contains(&format!("src:url('{relative}')")));
    assert!(!css.contains("data:font/"));
    assert_eq!(fs::read(output.join(relative)).unwrap(), TEST_FONT[0].data);
    let _ = fs::remove_dir_all(temp);
}

#[test]
fn static_site_embeds_fonts_only_when_inline_is_enabled() {
    let temp = std::env::temp_dir().join(format!(
        "fission-site-font-inline-test-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(temp.join("content")).unwrap();
    let mut options = SiteBuildOptions::for_project(&temp, "Test site");
    options.content_routes = Vec::new();
    let site = FissionSite::new()
        .with_fonts(&TEST_FONT)
        .with_font_delivery(crate::WebFontDelivery::Inline)
        .route_widget::<TestState, _>("/", "Home", None, Text::new("Home"));

    build_site(&options, &site).unwrap();

    let output = temp.join("target/fission/site");
    let css = fs::read_to_string(output.join("site.css")).unwrap();
    let relative = packaged_font_asset_href(&TEST_FONT[0]);
    assert!(css.contains("src:url('data:font/woff2;base64,"));
    assert!(!output.join(relative).exists());
    let _ = fs::remove_dir_all(temp);
}

#[test]
fn static_protected_route_emits_redirect_without_allowed_content() {
    let temp = std::env::temp_dir().join(format!(
        "fission-site-protected-route-test-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(temp.join("content")).unwrap();
    let mut options = SiteBuildOptions::for_project(&temp, "Test site");
    options.content_routes = Vec::new();
    let site = FissionSite::new()
        .route_widget::<TestState, _>(
            "/account",
            "Account",
            None,
            ProtectedRoute::new(
                fission_core::RouteRedirect::replace("/sign-in?from=account").into(),
                Text::new("private account"),
            ),
        )
        .route_widget::<TestState, _>("/sign-in", "Sign in", None, Text::new("Sign in"));

    build_site(&options, &site).unwrap();
    let html = fs::read_to_string(temp.join("target/fission/site/account/index.html")).unwrap();

    assert!(html.contains("<meta http-equiv=\"refresh\" content=\"0; url=/sign-in?from=account\">"));
    assert!(!html.contains("private account"));
    let _ = fs::remove_dir_all(temp);
}

#[test]
fn content_front_matter_selects_locale_without_path_conventions() {
    let temp = std::env::temp_dir().join(format!(
        "fission-site-front-locale-test-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(temp.join("content")).unwrap();
    fs::write(
        temp.join("content/catalog.md"),
        "---\ntitle: Catalog\nlocale: es-ES\n---\n# Catalog",
    )
    .unwrap();
    let options = SiteBuildOptions::for_project(&temp, "Test site");

    build_site(&options, &FissionSite::new()).unwrap();
    let html =
        fs::read_to_string(temp.join("target/fission/site/content/catalog/index.html")).unwrap();

    assert!(html.contains("lang=\"es-ES\""));
    assert!(html.contains("property=\"og:locale\" content=\"es_ES\""));
    let _ = fs::remove_dir_all(temp);
}

#[test]
fn custom_routes_support_build_state_localized_metadata_and_structured_data() {
    let temp = std::env::temp_dir().join(format!(
        "fission-site-route-parity-test-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(temp.join("content")).unwrap();
    let mut options = SiteBuildOptions::for_project(&temp, "Test site");
    options.content_routes = Vec::new();
    let site = FissionSite::new()
        .default_locale("en")
        .locale_resolver(|ctx| {
            Ok(if ctx.route_path == "/catalog/" {
                "fr".into()
            } else {
                ctx.default_locale.into()
            })
        })
        .document_metadata(|ctx, defaults| {
            Ok(crate::DocumentMetadata::new(
                format!("{} ({})", defaults.title, ctx.env().locale.0),
                defaults.description.clone(),
            ))
        })
        .route_widget_with_state::<RouteState, _, _>(
            "/catalog/",
            "Catalog",
            None,
            RouteStatePage,
            |ctx| Ok(RouteState(format!("built {}", ctx.route_path))),
        )
        .with_route_structured_data("/catalog/", [r#"{"@type":"CollectionPage"}"#]);

    build_site(&options, &site).unwrap();
    let html = fs::read_to_string(temp.join("target/fission/site/catalog/index.html")).unwrap();

    assert!(html.contains("built /catalog/"));
    assert!(html.contains("lang=\"fr\""));
    assert!(html.contains("<title>Catalog (fr)</title>"));
    assert!(html.contains("CollectionPage"));
    let _ = fs::remove_dir_all(temp);
}

#[test]
fn custom_site_routes_render_video_as_html_video() {
    let temp = std::env::temp_dir().join(format!(
        "fission-site-video-test-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(&temp).unwrap();
    fs::create_dir_all(temp.join("content")).unwrap();
    let mut options = SiteBuildOptions::for_project(&temp, "Test site");
    options.content_routes = Vec::new();
    let site = FissionSite::new().route_widget::<TestState, _>("/", "Video", None, VideoPage);

    build_site(&options, &site).unwrap();
    let html = fs::read_to_string(temp.join("target/fission/site/index.html")).unwrap();

    assert!(html.contains("<video"));
    assert!(html.contains("src=\"https://example.com/demo.mp4\""));
    assert!(html.contains("autoplay muted"));
    assert!(html.contains("loop"));
    let _ = fs::remove_dir_all(temp);
}

#[test]
fn custom_site_routes_preserve_registered_portals() {
    let temp = std::env::temp_dir().join(format!(
        "fission-site-portal-test-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(temp.join("content")).unwrap();
    let mut options = SiteBuildOptions::for_project(&temp, "Test site");
    options.content_routes = Vec::new();
    let site = FissionSite::new().route_widget::<TestState, _>("/", "Portal", None, PortalPage);

    build_site(&options, &site).unwrap();
    let html = fs::read_to_string(temp.join("target/fission/site/index.html")).unwrap();

    assert!(html.contains("Static portal root"));
    assert!(html.contains("Static portal overlay"));
    let _ = fs::remove_dir_all(temp);
}

#[test]
fn project_site_nav_supports_nested_dropdowns() {
    let temp = std::env::temp_dir().join(format!(
        "fission-site-nav-test-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(&temp).unwrap();
    fs::write(
        temp.join("fission.toml"),
        r#"
[app]
name = "docs"

[site]
title = "Docs"

[[site.nav]]
title = "Product"
href = "product/overview"

[[site.nav.children]]
title = "Resources"
href = "/docs/"

[[site.nav.children.children]]
title = "Documentation"
href = "/docs/reference/"
"#,
    )
    .unwrap();

    let options = SiteBuildOptions::from_project_dir(&temp, "Docs").unwrap();
    assert_eq!(options.site_nav.len(), 1);
    assert_eq!(options.site_nav[0].href, "/product/overview/");
    assert_eq!(options.site_nav[0].children[0].title, "Resources");
    assert_eq!(
        options.site_nav[0].children[0].children[0].href,
        "/docs/reference/"
    );
    let _ = fs::remove_dir_all(temp);
}

#[test]
fn blog_template_generates_routes_for_the_configured_mount() {
    let temp = std::env::temp_dir().join(format!(
        "fission-site-blog-test-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(temp.join("content/blog")).unwrap();
    fs::write(
        temp.join("content/blog/2026-06-02-release.md"),
        "---\ntitle: Release post\ntags:\n  - release\ncategories: [updates]\n---\n# Release post\n\n{/* truncate */}\n\nPost body.",
    )
    .unwrap();

    let mut options = SiteBuildOptions::for_project(&temp, "Test site");
    options.content_routes = vec![SiteContentRouteConfig {
        path: "/journal".to_string(),
        source: temp.join("content/blog"),
        template: Some("fission::site::blog".to_string()),
        sidebar: None,
    }];

    let report = build_content_site(&options).unwrap();
    assert!(report.routes.iter().any(|route| route.path == "/journal/"));
    assert!(report
        .routes
        .iter()
        .any(|route| route.path == "/journal/2026-06-02-release/"));
    assert!(report
        .routes
        .iter()
        .any(|route| route.path == "/journal/categories/updates/"));
    assert!(report
        .routes
        .iter()
        .any(|route| route.path == "/journal/tags/release/"));

    let index = fs::read_to_string(temp.join("target/fission/site/journal/index.html")).unwrap();
    assert!(index.contains("Featured"));
    assert!(index.contains("Release post"));
    assert!(index.contains("release"));
    assert!(index.contains("updates"));
    assert!(index.contains("journal/categories/updates/"));
    assert!(index.contains("journal/tags/release/"));

    let post =
        fs::read_to_string(temp.join("target/fission/site/journal/2026-06-02-release/index.html"))
            .unwrap();
    assert!(post.contains("Post"));
    assert!(post.contains("body."));
    assert!(!post.contains("truncate"));
    assert!(post.contains("site-blog-adjacent-posts") || !post.contains("Older post"));
    assert!(post.contains("journal/categories/updates/"));
    assert!(post.contains("journal/tags/release/"));

    let category =
        fs::read_to_string(temp.join("target/fission/site/journal/categories/updates/index.html"))
            .unwrap();
    assert!(category.contains("Category: updates"));
    assert!(category.contains("Release post"));

    let tag = fs::read_to_string(temp.join("target/fission/site/journal/tags/release/index.html"))
        .unwrap();
    assert!(tag.contains("Tag: #release"));
    assert!(tag.contains("Release post"));
    let _ = fs::remove_dir_all(temp);
}

#[test]
fn blog_pagination_does_not_leak_into_documentation_mounts() {
    let temp = std::env::temp_dir().join(format!(
        "fission-site-mixed-content-test-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(temp.join("content/docs")).unwrap();
    fs::create_dir_all(temp.join("content/blog")).unwrap();
    fs::write(
        temp.join("content/docs/index.md"),
        "---\ntitle: Documentation\n---\n# Documentation\n\nStart here.",
    )
    .unwrap();
    for index in 1..=9 {
        fs::write(
            temp.join(format!("content/blog/2026-06-{index:02}-post.md")),
            format!("---\ntitle: Post {index}\n---\n# Post {index}\n\nPost body."),
        )
        .unwrap();
    }

    let mut options = SiteBuildOptions::for_project(&temp, "Test site");
    options.content_routes = vec![
        SiteContentRouteConfig {
            path: "/docs".to_string(),
            source: temp.join("content/docs"),
            template: Some("fission::site::documentation".to_string()),
            sidebar: None,
        },
        SiteContentRouteConfig {
            path: "/journal".to_string(),
            source: temp.join("content/blog"),
            template: Some("fission::site::blog".to_string()),
            sidebar: None,
        },
    ];

    let report = build_content_site(&options).unwrap();
    assert!(report
        .routes
        .iter()
        .any(|route| route.path == "/journal/page/2/"));
    let docs = fs::read_to_string(temp.join("target/fission/site/docs/index.html")).unwrap();
    assert!(!docs.contains("docs/page/2/"));
    let _ = fs::remove_dir_all(temp);
}

#[test]
fn content_header_widget_replaces_the_default_content_header() {
    let temp = std::env::temp_dir().join(format!(
        "fission-site-content-header-test-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(temp.join("content/docs")).unwrap();
    fs::write(
        temp.join("content/docs/index.md"),
        "---\ntitle: Guide\n---\n# Guide\n\nContent body.",
    )
    .unwrap();

    let mut options = SiteBuildOptions::for_project(&temp, "Test site");
    options.content_routes = vec![SiteContentRouteConfig {
        path: "/docs".to_string(),
        source: temp.join("content/docs"),
        template: Some("fission::site::documentation".to_string()),
        sidebar: None,
    }];
    let site = FissionSite::new().content_header_widget::<TestState, _, _>(|ctx| {
        Text::new(format!("Custom header for {}", ctx.route_path))
    });

    build_site(&options, &site).unwrap();
    let html = fs::read_to_string(temp.join("target/fission/site/docs/index.html")).unwrap();

    assert!(html.contains("Custom header for /docs/"));
    assert!(!html.contains("fission-site-doc-header"));
    let _ = fs::remove_dir_all(temp);
}

#[test]
fn relative_markdown_links_are_resolved_to_site_routes() {
    let temp = std::env::temp_dir().join(format!(
        "fission-site-link-test-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let content = temp.join("content/reference/charts/bar");
    fs::create_dir_all(&content).unwrap();
    fs::write(content.join("overview.mdx"), "# Bar").unwrap();
    fs::write(content.join("bar-ranked.mdx"), "# Ranked").unwrap();

    let source_file = content.join("bar-ranked.mdx");
    let resolved = resolve_relative_markdown_links(
        "[Bar family overview](./overview) and [Ranked](./bar-ranked#example)",
        "/reference",
        &temp.join("content/reference"),
        &source_file,
    );

    assert_eq!(
        resolved,
        "[Bar family overview](/reference/charts/bar/overview/) and [Ranked](/reference/charts/bar/bar-ranked/#example)"
    );
    let _ = fs::remove_dir_all(temp);
}
