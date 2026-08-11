use super::{
    build_content_site, build_site,
    routing::{resolve_relative_markdown_links, route_path_from_file, route_path_from_slug},
    site_base_css, site_enhancement_js, SiteBuildOptions, SiteContentRouteConfig,
};
use crate::document::SiteNavLink;
use crate::site::{FissionSite, SitePageElement};
use fission_core::ui::{Text, TextContent, Video};
use fission_core::{Env, GlobalState, Widget, WidgetId};
use fission_i18n::TranslationBundle;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Default)]
struct TestState;
impl GlobalState for TestState {}

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
    assert!(css.contains(".fission-site-svg-colored svg *"));
    assert!(css.contains("fill: currentColor;"));
    assert!(css.contains(".fission-site-svg-colored svg [fill=\"none\"]"));
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
    let css = fs::read_to_string(temp.join("target/fission/site/site.css")).unwrap();
    assert!(css.contains(":root"));
    assert!(css.contains(".fs_"));
    assert!(css.contains(".fission-doc-tabs"));
    assert!(temp.join("target/fission/site/sitemap.xml").exists());
    assert!(temp.join("target/fission/site/robots.txt").exists());
    assert!(temp.join("target/fission/site/search/search.js").exists());
    assert!(temp
        .join("target/fission/site/search/manifest.json")
        .exists());
    let docs = fs::read_to_string(temp.join("target/fission/site/search/docs.json")).unwrap();
    assert!(docs.contains("Getting started"));
    let _ = fs::remove_dir_all(temp);
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
fn blog_routes_get_generated_index_and_strip_truncate_marker() {
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
        path: "/blog".to_string(),
        source: temp.join("content/blog"),
        template: None,
        sidebar: None,
    }];

    let report = build_content_site(&options).unwrap();
    assert!(report.routes.iter().any(|route| route.path == "/blog/"));
    assert!(report
        .routes
        .iter()
        .any(|route| route.path == "/blog/2026-06-02-release/"));
    assert!(report
        .routes
        .iter()
        .any(|route| route.path == "/blog/categories/updates/"));
    assert!(report
        .routes
        .iter()
        .any(|route| route.path == "/blog/tags/release/"));

    let index = fs::read_to_string(temp.join("target/fission/site/blog/index.html")).unwrap();
    assert!(index.contains("Featured"));
    assert!(index.contains("Release post"));
    assert!(index.contains("release"));
    assert!(index.contains("updates"));
    assert!(index.contains("blog/categories/updates/"));
    assert!(index.contains("blog/tags/release/"));

    let post =
        fs::read_to_string(temp.join("target/fission/site/blog/2026-06-02-release/index.html"))
            .unwrap();
    assert!(post.contains("Post"));
    assert!(post.contains("body."));
    assert!(!post.contains("truncate"));
    assert!(post.contains("site-blog-adjacent-posts") || !post.contains("Older post"));
    assert!(post.contains("blog/categories/updates/"));
    assert!(post.contains("blog/tags/release/"));

    let category =
        fs::read_to_string(temp.join("target/fission/site/blog/categories/updates/index.html"))
            .unwrap();
    assert!(category.contains("Category: updates"));
    assert!(category.contains("Release post"));

    let tag =
        fs::read_to_string(temp.join("target/fission/site/blog/tags/release/index.html")).unwrap();
    assert!(tag.contains("Tag: #release"));
    assert!(tag.contains("Release post"));
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
