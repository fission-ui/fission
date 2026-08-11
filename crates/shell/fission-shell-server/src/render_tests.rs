use super::*;
use crate::{
    CacheError, CacheTag, InvalidationReport, MokaCache, ProgressiveWorker, RevalidationPolicy,
    WasmIsland, WebRouteMode,
};
use fission_core::ui::{Button, SemanticsRegion, Text, TextContent};
use fission_core::{
    Action, ActionId, GlobalState, Handler, JobRef, JobResource, JobSpec, ReducerContext,
    ResourceKey, Role, Widget, WidgetId,
};
use fission_i18n::TranslationBundle;
use fission_theme::{PackagedFont, PackagedFontStyle};
use fission_widgets::MarkdownViewer;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

#[derive(Debug, Default)]
struct TestState;
impl GlobalState for TestState {}

#[derive(Debug, Default)]
struct PathState {
    route_path: String,
}
impl GlobalState for PathState {}

#[derive(Clone)]
struct TestPage(&'static str);

impl From<TestPage> for Widget {
    fn from(component: TestPage) -> Self {
        let (_ctx, _view) = fission_core::build::current::<TestState>();
        Text::new(component.0).into()
    }
}

#[derive(Clone)]
struct ThemeTogglePage;

impl From<ThemeTogglePage> for Widget {
    fn from(_: ThemeTogglePage) -> Self {
        let (_ctx, _view) = fission_core::build::current::<TestState>();
        SemanticsRegion::new(Text::new("Cambiar tema"))
            .identifier("site-theme-toggle")
            .label("Cambiar el tema de color")
            .role(Role::Button)
            .into()
    }
}

#[derive(Clone)]
struct PortalPage;

impl From<PortalPage> for Widget {
    fn from(_: PortalPage) -> Self {
        let (ctx, _view) = fission_core::build::current::<TestState>();
        ctx.register_portal(Text::new("Portal overlay").into());
        Text::new("Root content").into()
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
struct CodePage;

impl From<CodePage> for Widget {
    fn from(_: CodePage) -> Self {
        MarkdownViewer {
            markdown: "```rust\nlet answer = 42;\n```".to_string(),
            show_scrollbar: false,
        }
        .into()
    }
}

const TEST_FONT: [PackagedFont; 1] = [PackagedFont {
    family: "Parity Sans",
    weight: 400,
    style: PackagedFontStyle::Normal,
    format: "woff2",
    data: b"font-bytes",
    axes: &[],
}];

#[derive(Clone)]
struct PathPage;

impl From<PathPage> for Widget {
    fn from(_: PathPage) -> Self {
        let (_ctx, view) = fission_core::build::current::<PathState>();
        Text::new(view.state().route_path.clone()).into()
    }
}

fn translated_env() -> Env {
    let mut env = Env::default();
    env.i18n.add_bundle(TranslationBundle {
        locale: "en".into(),
        messages: HashMap::from([
            ("page.title".to_string(), "Hello SSR".to_string()),
            ("catalog.title".to_string(), "Catalog".to_string()),
        ]),
    });
    env.i18n.add_bundle(TranslationBundle {
        locale: "fr".into(),
        messages: HashMap::from([
            ("page.title".to_string(), "Bonjour SSR".to_string()),
            ("catalog.title".to_string(), "Catalogue".to_string()),
        ]),
    });
    env
}

#[derive(Clone)]
struct TestActionPage;

impl From<TestActionPage> for Widget {
    fn from(_component: TestActionPage) -> Self {
        let (_ctx, _view) = fission_core::build::current::<TestState>();
        Button {
            child: Some(Text::new("Run action").into()),
            on_press: Some(ActionEnvelope::from(TestAction)),
            ..Default::default()
        }
        .into()
    }
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct TestAction;

impl Action for TestAction {
    fn static_id() -> ActionId {
        ActionId::from_name("server-renderer.test-action")
    }
}

#[derive(Debug, Default)]
struct ActionJobState {
    message: Option<String>,
}

impl GlobalState for ActionJobState {}

#[derive(Debug)]
struct ActionJob;

#[derive(Clone, Debug, Serialize, Deserialize)]
struct ActionJobRequest;

impl JobSpec for ActionJob {
    type Request = ActionJobRequest;
    type Ok = String;
    type Err = String;

    const NAME: &'static str = "server-renderer.action-job";
}

const ACTION_JOB: JobRef<ActionJob> = JobRef::new(ActionJob::NAME);

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct StartActionJob;

impl Action for StartActionJob {
    fn static_id() -> ActionId {
        ActionId::from_name("server-renderer.start-action-job")
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct ActionJobLoaded;

impl Action for ActionJobLoaded {
    fn static_id() -> ActionId {
        ActionId::from_name("server-renderer.action-job-loaded")
    }
}

fn on_start_action_job(
    _state: &mut ActionJobState,
    _action: StartActionJob,
    ctx: &mut ReducerContext<ActionJobState>,
) {
    ctx.effects
        .app(ACTION_JOB, ActionJobRequest)
        .on_ok(ActionEnvelope::from(ActionJobLoaded))
        .dispatch();
}

fn on_action_job_loaded(
    state: &mut ActionJobState,
    _action: ActionJobLoaded,
    ctx: &mut ReducerContext<ActionJobState>,
) {
    state.message = ctx.input.job_ok(ACTION_JOB);
}

#[derive(Clone)]
struct ActionJobPage;

impl From<ActionJobPage> for Widget {
    fn from(_: ActionJobPage) -> Self {
        let (ctx, view) = fission_core::build::current::<ActionJobState>();
        ctx.register::<ActionJobLoaded, _>(
            on_action_job_loaded as Handler<ActionJobState, ActionJobLoaded>,
        );
        let on_press = ctx.bind(
            StartActionJob,
            on_start_action_job as Handler<ActionJobState, StartActionJob>,
        );
        Button {
            id: Some(WidgetId::explicit("action-job-button")),
            child: Some(
                Text::new(
                    view.state()
                        .message
                        .clone()
                        .unwrap_or_else(|| "Pending job".to_string()),
                )
                .into(),
            ),
            on_press: Some(on_press),
            ..Default::default()
        }
        .into()
    }
}

struct CountingCache {
    inner: MokaCache,
    puts: AtomicUsize,
}

impl CountingCache {
    fn new() -> Self {
        Self {
            inner: MokaCache::default(),
            puts: AtomicUsize::new(0),
        }
    }

    fn put_count(&self) -> usize {
        self.puts.load(Ordering::SeqCst)
    }
}

impl Cache for CountingCache {
    fn get(&self, key: &CacheKey) -> Result<Option<CacheEntry>, CacheError> {
        self.inner.get(key)
    }

    fn put(&self, entry: CacheEntry) -> Result<(), CacheError> {
        self.puts.fetch_add(1, Ordering::SeqCst);
        self.inner.put(entry)
    }

    fn remove(&self, key: &CacheKey) -> Result<(), CacheError> {
        self.inner.remove(key)
    }

    fn invalidate_tag(&self, tag: &CacheTag) -> Result<InvalidationReport, CacheError> {
        self.inner.invalidate_tag(tag)
    }
}

fn default_render_env(renderer: &ServerRenderer) -> Env {
    let mut env = renderer.app.document.env().clone();
    env.theme = renderer.app.document.theme().clone();
    env.locale = renderer.default_locale.as_str().into();
    env
}

#[test]
fn server_renderer_resolves_keyed_text_from_seeded_env() {
    let app = FissionServerApp::new("Test")
        .with_env(translated_env())
        .route_widget::<TestState, _>(
            "/",
            "Home",
            None,
            WebRouteMode::Server(Default::default()),
            KeyPage("page.title"),
        );
    let renderer = ServerRenderer::new(app);

    let response = renderer.handle(ServerRequest::get("/")).unwrap();

    assert_eq!(response.status, 200);
    assert!(response.body_string().contains("Hello SSR"));
    assert!(!response.body_string().contains("MISSING:page.title"));
}

#[test]
fn server_renderer_mounts_portals_into_ssr_html() {
    let renderer = ServerRenderer::new(
        FissionServerApp::new("Test")
            .server_route_widget::<TestState, _>("/", "Home", None, PortalPage),
    );

    let response = renderer.handle(ServerRequest::get("/")).unwrap();
    let body = response.body_string();

    assert_eq!(response.status, 200);
    assert!(body.contains("Root content"));
    assert!(body.contains("Portal overlay"));
}

#[test]
fn server_actions_drain_supported_job_effects_before_rendering_response() {
    let renderer = ServerRenderer::new(
        FissionServerApp::new("Test")
            .jobs(
                ServerJobRegistry::new()
                    .register_job(ACTION_JOB, |_request, _ctx| Ok("Job complete".to_string())),
            )
            .server_route_widget::<ActionJobState, _>("/", "Home", None, ActionJobPage),
    );
    let token = renderer.sign_action(
        "/",
        WidgetId::explicit("action-job-button").as_u128(),
        StartActionJob,
        Duration::from_secs(60),
    );
    let request = ServerRequest::post("/__fission/action", serde_json::to_vec(&token).unwrap());

    let response = renderer.handle(request).unwrap();

    assert_eq!(response.status, 200);
    assert!(response.body_string().contains("Job complete"));
}

#[test]
fn server_renderer_uses_request_env_locale_for_html_and_text() {
    let app = FissionServerApp::new("Test")
        .with_env(translated_env())
        .with_request_env(|ctx, env| {
            if ctx.route_path.starts_with("/fr") {
                env.locale = "fr".into();
            }
            Ok(())
        })
        .route_widget::<TestState, _>(
            "/fr",
            "Home",
            None,
            WebRouteMode::Server(Default::default()),
            KeyPage("page.title"),
        );
    let renderer = ServerRenderer::new(app);

    let response = renderer.handle(ServerRequest::get("/fr")).unwrap();
    let body = response.body_string();

    assert_eq!(response.status, 200);
    assert!(body.contains("Bonjour SSR"));
    assert!(body.contains("lang=\"fr\""));
}

#[test]
fn document_metadata_resolver_uses_the_request_locale() {
    let app = FissionServerApp::new("Test")
        .with_env(translated_env())
        .with_request_env(|ctx, env| {
            if ctx.route_path.starts_with("/fr") {
                env.locale = "fr".into();
            }
            Ok(())
        })
        .document_metadata(|ctx, route| {
            let title = ctx
                .env()
                .i18n
                .get(&ctx.env().locale, &route.title)
                .unwrap_or(&route.title)
                .to_string();
            Ok(crate::ServerDocumentMetadata::new(
                title,
                Some(format!("{} description", ctx.env().locale.0)),
            ))
        })
        .route_widget::<TestState, _>(
            "/fr",
            "page.title",
            None,
            WebRouteMode::Server(Default::default()),
            KeyPage("page.title"),
        );
    let renderer = ServerRenderer::new(app);

    let body = renderer
        .handle(ServerRequest::get("/fr"))
        .unwrap()
        .body_string();

    assert!(body.contains("<title>Bonjour SSR</title>"));
    assert!(body.contains("content=\"fr description\""));
}

#[test]
fn server_app_registers_i18n_bundle_and_default_locale() {
    let app = FissionServerApp::new("Test")
        .translation_bundle(TranslationBundle {
            locale: "fr".into(),
            messages: HashMap::from([("page.title".to_string(), "Bonjour SSR".to_string())]),
        })
        .default_locale("fr")
        .route_widget::<TestState, _>(
            "/",
            "Home",
            None,
            WebRouteMode::Server(Default::default()),
            KeyPage("page.title"),
        );
    let renderer = ServerRenderer::new(app);

    let response = renderer.handle(ServerRequest::get("/")).unwrap();
    let body = response.body_string();

    assert_eq!(response.status, 200);
    assert!(body.contains("Bonjour SSR"));
    assert!(body.contains("lang=\"fr\""));
}

#[test]
fn locale_resolver_can_use_dynamic_route_params() {
    let app = FissionServerApp::new("Test")
        .with_env(translated_env())
        .locale_resolver(|ctx| Ok(ctx.route_params["locale"].as_str().into()))
        .route_widget::<TestState, _>(
            "/locale/:locale/catalog",
            "Catalog",
            None,
            WebRouteMode::Server(Default::default()),
            KeyPage("catalog.title"),
        );
    let renderer = ServerRenderer::new(app);

    let response = renderer
        .handle(ServerRequest::get("/locale/fr/catalog"))
        .unwrap();
    let body = response.body_string();

    assert_eq!(response.status, 200);
    assert!(body.contains("Catalogue"));
    assert!(body.contains("lang=\"fr\""));
}

#[test]
fn prefix_routes_receive_concrete_request_path() {
    let app = FissionServerApp::new("Test").route_prefix_widget_with_state::<PathState, _, _>(
        "/docs/",
        "Docs",
        None,
        WebRouteMode::Server(Default::default()),
        PathPage,
        |ctx| {
            Ok(PathState {
                route_path: ctx.route_path.to_string(),
            })
        },
    );
    let renderer = ServerRenderer::new(app);

    let response = renderer
        .handle(ServerRequest::get("/docs/platform"))
        .unwrap();

    assert_eq!(response.status, 200);
    assert!(response.body_string().contains("/docs/platform/"));
}

#[test]
fn dynamic_routes_expose_route_params() {
    let app = FissionServerApp::new("Test").route_widget_with_state::<PathState, _, _>(
        "/items/:item_id",
        "Item",
        None,
        WebRouteMode::Server(Default::default()),
        PathPage,
        |ctx| {
            Ok(PathState {
                route_path: ctx.route_params["item_id"].clone(),
            })
        },
    );
    let renderer = ServerRenderer::new(app);

    let response = renderer.handle(ServerRequest::get("/items/42")).unwrap();

    assert_eq!(response.status, 200);
    assert!(response.body_string().contains("42"));
}

#[test]
fn exact_routes_win_over_dynamic_routes() {
    let app = FissionServerApp::new("Test")
        .route_widget::<TestState, _>(
            "/items/new",
            "New",
            None,
            WebRouteMode::Server(Default::default()),
            TestPage("exact new item page"),
        )
        .route_widget_with_state::<PathState, _, _>(
            "/items/:item_id",
            "Item",
            None,
            WebRouteMode::Server(Default::default()),
            PathPage,
            |ctx| {
                Ok(PathState {
                    route_path: ctx.route_params["item_id"].clone(),
                })
            },
        );
    let renderer = ServerRenderer::new(app);

    let response = renderer.handle(ServerRequest::get("/items/new")).unwrap();

    assert_eq!(response.status, 200);
    assert!(response.body_string().contains("exact new item page"));
    assert!(!response.body_string().contains(">new<"));
}

#[test]
fn route_renderers_can_set_http_response_status() {
    let app = FissionServerApp::new("Test").route_prefix_widget_with_state::<PathState, _, _>(
        "/docs/",
        "Docs",
        None,
        WebRouteMode::Server(Default::default()),
        PathPage,
        |ctx| {
            ctx.set_response_status(404);
            Ok(PathState {
                route_path: ctx.route_path.to_string(),
            })
        },
    );
    let renderer = ServerRenderer::new(app);

    let rendered = renderer.render_route("/docs/missing").unwrap();
    assert_eq!(rendered.status, 404);
    assert!(rendered.html.contains("/docs/missing/"));

    let response = renderer
        .handle(ServerRequest::get("/docs/missing"))
        .unwrap();
    assert_eq!(response.status, 404);
    assert!(response.body_string().contains("/docs/missing/"));
}

#[test]
fn exact_routes_win_over_prefix_routes() {
    let app = FissionServerApp::new("Test")
        .route_prefix_widget_with_state::<PathState, _, _>(
            "/docs/",
            "Docs",
            None,
            WebRouteMode::Server(Default::default()),
            PathPage,
            |ctx| {
                Ok(PathState {
                    route_path: ctx.route_path.to_string(),
                })
            },
        )
        .route_widget::<TestState, _>(
            "/docs/index/",
            "Exact",
            None,
            WebRouteMode::Server(Default::default()),
            TestPage("exact docs page"),
        );
    let renderer = ServerRenderer::new(app);

    let response = renderer.handle(ServerRequest::get("/docs/index")).unwrap();

    assert_eq!(response.status, 200);
    assert!(response.body_string().contains("exact docs page"));
    assert!(!response.body_string().contains("/docs/index/"));
}

#[test]
fn http_handlers_can_use_blocking_clients_inside_tokio_runtime() {
    let app = FissionServerApp::new("Test").form_post("/submit", |_ctx| {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap();
        let value = runtime.block_on(async { "stored" });
        Ok(ServerResponse::text(
            200,
            "text/plain; charset=utf-8",
            value,
        ))
    });
    let renderer = ServerRenderer::new(app);
    let runtime = tokio::runtime::Runtime::new().unwrap();

    let response = runtime
        .block_on(async { renderer.handle(ServerRequest::post("/submit", "email=a@b.test")) })
        .unwrap();

    assert_eq!(response.status, 200);
    assert_eq!(response.body_string(), "stored");
}

#[test]
fn route_state_loaders_can_use_blocking_clients_inside_tokio_runtime() {
    let app = FissionServerApp::new("Test").route_widget_with_state::<TestState, _, _>(
        "/blocking",
        "Blocking",
        None,
        WebRouteMode::Server(Default::default()),
        TestPage("blocking state loaded"),
        |_ctx| {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .build()
                .unwrap();
            runtime.block_on(async { Ok(TestState) })
        },
    );
    let renderer = ServerRenderer::new(app);
    let runtime = tokio::runtime::Runtime::new().unwrap();

    let response = runtime
        .block_on(async { renderer.handle(ServerRequest::get("/blocking")) })
        .unwrap();

    assert_eq!(response.status, 200);
    assert!(response.body_string().contains("blocking state loaded"));
}

#[test]
fn revalidated_cache_keys_vary_by_resolved_locale() {
    let cache = Arc::new(MokaCache::default());
    let app = FissionServerApp::new("Test")
        .with_env(translated_env())
        .with_request_env(|ctx, env| {
            if header_value(&ctx.request.headers, "accept-language")
                .is_some_and(|value| value.starts_with("fr"))
            {
                env.locale = "fr".into();
            }
            Ok(())
        })
        .route_widget::<TestState, _>(
            "/catalog",
            "Catalog",
            None,
            WebRouteMode::Revalidated(
                RevalidationPolicy::new(Duration::from_secs(60)).tag("catalog"),
            ),
            KeyPage("catalog.title"),
        );
    let renderer = ServerRenderer::new(app).with_cache(cache);
    let en = ServerRequest::get("/catalog");
    let mut fr = ServerRequest::get("/catalog");
    fr.headers
        .insert("accept-language".to_string(), "fr-FR".to_string());

    let en_first = renderer.handle(en.clone()).unwrap();
    let fr_first = renderer.handle(fr).unwrap();
    let en_second = renderer.handle(en).unwrap();

    assert_eq!(en_first.cache_status, Some(Freshness::Expired));
    assert!(en_first.body_string().contains("Catalog"));
    assert_eq!(fr_first.cache_status, Some(Freshness::Expired));
    assert!(fr_first.body_string().contains("Catalogue"));
    assert_eq!(en_second.cache_status, Some(Freshness::Fresh));
    assert!(en_second.body_string().contains("Catalog"));
}

#[test]
fn renderer_exposes_direct_and_helper_cache_invalidation() {
    let app = FissionServerApp::new("Test").route_widget::<TestState, _>(
        "/posts",
        "Posts",
        None,
        WebRouteMode::Revalidated(
            RevalidationPolicy::new(Duration::from_secs(60)).tags(["posts", "post:1"]),
        ),
        TestPage("Posts"),
    );
    let renderer = ServerRenderer::new(app);

    assert_eq!(
        renderer
            .handle(ServerRequest::get("/posts"))
            .unwrap()
            .cache_status,
        Some(Freshness::Expired)
    );
    let direct_report = renderer
        .cache()
        .invalidate_tag(&CacheTag::new("posts"))
        .unwrap();
    assert_eq!(direct_report.removed_keys, 1);
    assert_eq!(
        renderer
            .handle(ServerRequest::get("/posts"))
            .unwrap()
            .cache_status,
        Some(Freshness::Expired)
    );
    let helper_report = renderer.invalidate_cache_tag("post:1").unwrap();
    assert_eq!(helper_report.removed_keys, 1);
    assert_eq!(
        renderer
            .handle(ServerRequest::get("/posts"))
            .unwrap()
            .cache_status,
        Some(Freshness::Expired)
    );
}

#[test]
fn protected_cache_invalidation_endpoint_invalidates_tags() {
    let app = FissionServerApp::new("Test")
        .cache_invalidation_endpoint("/admin/cache/invalidate", "secret")
        .route_widget::<TestState, _>(
            "/posts",
            "Posts",
            None,
            WebRouteMode::Revalidated(
                RevalidationPolicy::new(Duration::from_secs(60)).tag("posts"),
            ),
            TestPage("Posts"),
        );
    let renderer = ServerRenderer::new(app);

    assert_eq!(
        renderer
            .handle(ServerRequest::get("/posts"))
            .unwrap()
            .cache_status,
        Some(Freshness::Expired)
    );
    assert_eq!(
        renderer
            .handle(ServerRequest::get("/posts"))
            .unwrap()
            .cache_status,
        Some(Freshness::Fresh)
    );

    let denied = renderer
        .handle(ServerRequest::post(
            "/admin/cache/invalidate",
            br#"{"tags":["posts"]}"#.to_vec(),
        ))
        .unwrap();
    assert_eq!(denied.status, 401);

    let mut request =
        ServerRequest::post("/admin/cache/invalidate", br#"{"tags":["posts"]}"#.to_vec());
    request
        .headers
        .insert("authorization".to_string(), "Bearer secret".to_string());
    let invalidated = renderer.handle(request).unwrap();

    assert_eq!(invalidated.status, 200);
    let report: InvalidationReport = serde_json::from_slice(&invalidated.body).unwrap();
    assert_eq!(report.removed_keys, 1);
    assert_eq!(report.layers_affected, 1);
    assert_eq!(
        renderer
            .handle(ServerRequest::get("/posts"))
            .unwrap()
            .cache_status,
        Some(Freshness::Expired)
    );
}

#[test]
fn server_renderer_caches_revalidated_routes() {
    let cache = Arc::new(MokaCache::default());
    let app = FissionServerApp::new("Test").route_widget::<TestState, _>(
        "/",
        "Home",
        None,
        WebRouteMode::Revalidated(RevalidationPolicy::new(Duration::from_secs(60)).tag("home")),
        TestPage("Hello cache"),
    );
    let renderer = ServerRenderer::new(app).with_cache(cache.clone());
    let env = default_render_env(&renderer);
    let key = renderer.cache_key_for_route(&renderer.routes()[0], &ServerRequest::get("/"), &env);

    let first = renderer.handle(ServerRequest::get("/")).unwrap();
    assert_eq!(first.status, 200);
    assert!(first.body_string().contains("Hello cache"));

    let second = renderer.handle(ServerRequest::get("/")).unwrap();
    assert_eq!(second.cache_status, Some(Freshness::Fresh));
    assert!(cache.contains_fresh(&key, SystemTime::now()).unwrap());
}

#[test]
fn revalidated_cache_key_normalizes_query_order() {
    let cache = Arc::new(MokaCache::default());
    let app = FissionServerApp::new("Test").route_widget::<TestState, _>(
        "/search",
        "Search",
        None,
        WebRouteMode::Revalidated(RevalidationPolicy::new(Duration::from_secs(60)).tag("search")),
        TestPage("Hello query cache"),
    );
    let renderer = ServerRenderer::new(app).with_cache(cache.clone());
    let mut first = ServerRequest::get("/search");
    first.query.insert("b".to_string(), "2".to_string());
    first.query.insert("a".to_string(), "1".to_string());
    let mut second = ServerRequest::get("/search");
    second.query.insert("a".to_string(), "1".to_string());
    second.query.insert("b".to_string(), "2".to_string());

    let env = default_render_env(&renderer);
    let first_key = renderer.cache_key_for_route(&renderer.routes()[0], &first, &env);
    assert_eq!(
        renderer.handle(first).unwrap().cache_status,
        Some(Freshness::Expired)
    );
    assert_eq!(
        renderer.handle(second).unwrap().cache_status,
        Some(Freshness::Fresh)
    );
    assert!(cache.contains_fresh(&first_key, SystemTime::now()).unwrap());
}

#[test]
fn revalidated_cache_key_includes_declared_vary_headers() {
    let cache = Arc::new(MokaCache::default());
    let app = FissionServerApp::new("Test").route_widget::<TestState, _>(
        "/catalog",
        "Catalog",
        None,
        WebRouteMode::Revalidated(
            RevalidationPolicy::new(Duration::from_secs(60)).vary("accept-language"),
        ),
        TestPage("Localized catalog"),
    );
    let renderer = ServerRenderer::new(app).with_cache(cache.clone());
    let mut en = ServerRequest::get("/catalog");
    en.headers
        .insert("accept-language".to_string(), "en-GB".to_string());
    let mut fr = ServerRequest::get("/catalog");
    fr.headers
        .insert("accept-language".to_string(), "fr-FR".to_string());

    let env = default_render_env(&renderer);
    let en_key = renderer.cache_key_for_route(&renderer.routes()[0], &en, &env);
    let fr_key = renderer.cache_key_for_route(&renderer.routes()[0], &fr, &env);
    assert_eq!(
        renderer.handle(en).unwrap().cache_status,
        Some(Freshness::Expired)
    );
    assert_eq!(
        renderer.handle(fr).unwrap().cache_status,
        Some(Freshness::Expired)
    );
    assert!(cache.contains_fresh(&en_key, SystemTime::now()).unwrap());
    assert!(cache.contains_fresh(&fr_key, SystemTime::now()).unwrap());
}

#[test]
fn cache_build_id_prefers_runtime_env_over_compile_time_env() {
    assert_eq!(
        cache_build_id_from(Some("release-42".to_string()), Some("compile-1"), "0.0.0"),
        "release-42"
    );
    assert_eq!(
        cache_build_id_from(Some("  ".to_string()), Some("compile-1"), "0.0.0"),
        "compile-1"
    );
    assert_eq!(cache_build_id_from(None, Some("  "), "0.0.0"), "0.0.0");
}

#[test]
fn private_server_routes_do_not_write_full_page_public_cache_entries() {
    let cache = Arc::new(CountingCache::new());
    let app = FissionServerApp::new("Test").route_widget::<TestState, _>(
        "/account",
        "Account",
        None,
        WebRouteMode::ServerPrivate(Default::default()),
        TestPage("Private account"),
    );
    let renderer = ServerRenderer::new(app).with_cache(cache.clone());

    let first = renderer.handle(ServerRequest::get("/account")).unwrap();
    let second = renderer.handle(ServerRequest::get("/account")).unwrap();

    assert_eq!(first.status, 200);
    assert_eq!(second.status, 200);
    assert_eq!(first.cache_status, None);
    assert_eq!(second.cache_status, None);
    assert_eq!(cache.put_count(), 0);
}

#[test]
fn server_renderer_rebuilds_expired_revalidated_routes() {
    let app = FissionServerApp::new("Test").route_widget::<TestState, _>(
        "/",
        "Home",
        None,
        WebRouteMode::Revalidated(RevalidationPolicy::new(Duration::from_millis(1)).tag("home")),
        TestPage("Hello rebuild"),
    );
    let renderer = ServerRenderer::new(app);

    let first = renderer.handle(ServerRequest::get("/")).unwrap();
    assert_eq!(first.cache_status, Some(Freshness::Expired));
    std::thread::sleep(Duration::from_millis(5));
    let second = renderer.handle(ServerRequest::get("/")).unwrap();
    assert_eq!(second.cache_status, Some(Freshness::Expired));
    assert!(second.body_string().contains("Hello rebuild"));
}

#[test]
fn revalidated_routes_reject_cached_server_action_tokens() {
    let app = FissionServerApp::new("Test").route_widget::<TestState, _>(
        "/",
        "Home",
        None,
        WebRouteMode::Revalidated(RevalidationPolicy::new(Duration::from_secs(60))),
        TestActionPage,
    );
    let renderer = ServerRenderer::new(app);

    let error = renderer.handle(ServerRequest::get("/")).unwrap_err();

    assert!(error.to_string().contains("renders server action forms"));
}

#[test]
fn configured_renderer_applies_fission_toml_server_settings() {
    let root = temp_project_dir("server-renderer-config");
    fs::create_dir_all(&root).unwrap();
    fs::write(
        root.join("fission.toml"),
        r#"[server]
default_route_mode = "revalidated"
render_pass_limit = 9

[server.cache]
provider = "moka"
max_capacity = 12
ttl = "2m"
stale_while_revalidate = "15s"
"#,
    )
    .unwrap();
    let app = FissionServerApp::new("Test")
        .project_dir(&root)
        .server_route_widget::<TestState, _>("/", "Home", None, TestPage("Configured"));

    let renderer = ServerRenderer::configured(app).unwrap();
    let routes = renderer.routes();

    assert!(matches!(
        routes.first().map(|route| &route.mode),
        Some(WebRouteMode::Revalidated(policy))
            if policy.ttl == Duration::from_secs(120)
                && policy.stale_while_revalidate == Some(Duration::from_secs(15))
    ));
    assert_eq!(renderer.render_pass_limit, 9);

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn configured_renderer_signs_session_cookie_when_secret_env_is_set() {
    let _guard = env_lock().lock().unwrap();
    let root = temp_project_dir("server-renderer-session-config");
    fs::create_dir_all(&root).unwrap();
    std::env::set_var("FISSION_TEST_SESSION_KEY", "test-session-secret");
    fs::write(
        root.join("fission.toml"),
        r#"[server]
default_route_mode = "server_private"

[server.sessions]
cookie_name = "shop_session"
signing_key_env = "FISSION_TEST_SESSION_KEY"
secure = true
same_site = "none"
"#,
    )
    .unwrap();
    let app = FissionServerApp::new("Test")
        .project_dir(&root)
        .server_route_widget::<TestState, _>("/", "Home", None, TestPage("Signed session"));
    let renderer = ServerRenderer::configured(app).unwrap();

    let response = renderer.handle(ServerRequest::get("/")).unwrap();
    let cookie = response_header(&response, "set-cookie")
        .unwrap()
        .to_string();
    let raw_value = cookie
        .split(';')
        .next()
        .unwrap()
        .strip_prefix("shop_session=")
        .unwrap()
        .to_string();
    assert_eq!(raw_value.split('.').count(), 2);

    let mut second = ServerRequest::get("/");
    second
        .headers
        .insert("cookie".to_string(), format!("shop_session={raw_value}"));
    let second = renderer.handle(second).unwrap();
    assert!(response_header(&second, "set-cookie").is_none());

    let mut tampered = ServerRequest::get("/");
    tampered.headers.insert(
        "cookie".to_string(),
        "shop_session=aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa.bad"
            .to_string(),
    );
    let tampered = renderer.handle(tampered).unwrap();
    assert!(response_header(&tampered, "set-cookie").is_some());

    std::env::remove_var("FISSION_TEST_SESSION_KEY");
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn trusted_proxy_headers_can_supply_canonical_url_when_enabled() {
    let app = FissionServerApp::new("Test").server_route_widget::<TestState, _>(
        "/docs",
        "Docs",
        None,
        TestPage("Docs"),
    );
    let mut renderer = ServerRenderer::new(app);
    renderer.http_config = ServerHttpConfig {
        base_url: None,
        trust_proxy_headers: true,
    };
    let mut request = ServerRequest::get("/docs");
    request
        .headers
        .insert("x-forwarded-proto".to_string(), "https".to_string());
    request
        .headers
        .insert("x-forwarded-host".to_string(), "fission.rs".to_string());

    let response = renderer.handle(request).unwrap();
    let html = response.body_string();

    assert!(html.contains(r#"rel="canonical" href="https://fission.rs/docs""#));
}

#[test]
fn route_manifest_includes_workers_and_islands() {
    let app = FissionServerApp::new("Test")
        .route_widget::<TestState, _>(
            "/",
            "Home",
            None,
            WebRouteMode::Server(Default::default()),
            TestPage("Interactive page"),
        )
        .worker(
            "/",
            ProgressiveWorker::new("filters", "/workers/filters.wasm"),
        )
        .island(
            "/",
            WasmIsland::new("cart", "/islands/cart.wasm", "cart-root"),
        );
    let renderer = ServerRenderer::new(app);
    let response = renderer.handle(ServerRequest::get("/")).unwrap();
    let html = response.body_string();
    assert!(html.contains("fission-route-manifest"));
    assert!(html.contains("src=\"/server-runtime.js\""));
    assert!(html.contains("filters"));
    assert!(html.contains("cart-root"));
}

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
    assert!(rendered.css.contains("font-family:'Parity Sans'"));
    assert!(rendered.css.contains("data:font/woff2;base64,"));
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

#[derive(Debug)]
struct MissingJob;

#[derive(Clone, Debug, Serialize, Deserialize)]
struct MissingJobRequest;

impl JobSpec for MissingJob {
    type Request = MissingJobRequest;
    type Ok = ();
    type Err = String;

    const NAME: &'static str = "server-renderer.missing-job";
}

const MISSING_JOB: JobRef<MissingJob> = JobRef::new(MissingJob::NAME);

#[derive(Clone)]
struct MissingJobPage;

impl From<MissingJobPage> for Widget {
    fn from(_component: MissingJobPage) -> Self {
        let (ctx, _) = fission_core::build::current::<TestState>();
        ctx.with_resources(|resources| {
            resources.job(JobResource::new(
                ResourceKey::new("missing-job"),
                MISSING_JOB,
                MissingJobRequest,
            ));
        });
        Text::new("Missing job").into()
    }
}
#[test]
fn server_rendering_rejects_unregistered_jobs_instead_of_silently_skipping_them() {
    let renderer = ServerRenderer::new(
        FissionServerApp::new("Test").server_route_widget::<TestState, _>(
            "/",
            "Home",
            None,
            MissingJobPage,
        ),
    );

    let error = renderer.handle(ServerRequest::get("/")).unwrap_err();
    assert!(error.to_string().contains("missing-job"));
}

#[derive(Debug, Default)]
struct LoopState {
    count: u32,
}

impl GlobalState for LoopState {}

#[derive(Debug)]
struct LoopJob;

#[derive(Clone, Debug, Serialize, Deserialize)]
struct LoopJobRequest {
    count: u32,
}

impl JobSpec for LoopJob {
    type Request = LoopJobRequest;
    type Ok = ();
    type Err = String;

    const NAME: &'static str = "server-renderer.loop-job";
}

const LOOP_JOB: JobRef<LoopJob> = JobRef::new(LoopJob::NAME);

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct LoopLoaded;

impl Action for LoopLoaded {
    fn static_id() -> ActionId {
        ActionId::from_name("server-renderer.loop-loaded")
    }
}

fn on_loop_loaded(
    state: &mut LoopState,
    _action: LoopLoaded,
    _ctx: &mut ReducerContext<LoopState>,
) {
    state.count = state.count.saturating_add(1);
}

#[derive(Clone)]
struct LoopPage;

impl From<LoopPage> for Widget {
    fn from(_component: LoopPage) -> Self {
        let (ctx, view) = fission_core::build::current::<LoopState>();
        let on_ok = ctx.bind(LoopLoaded, on_loop_loaded as Handler<LoopState, LoopLoaded>);
        ctx.with_resources(|resources| {
            resources.job(
                JobResource::new(
                    ResourceKey::new("loop-job"),
                    LOOP_JOB,
                    LoopJobRequest {
                        count: view.state().count,
                    },
                )
                .deps(view.state().count)
                .on_ok(on_ok),
            );
        });
        Text::new(format!("loop {}", view.state().count)).into()
    }
}
#[test]
fn server_rendering_fails_when_job_drain_exceeds_pass_limit() {
    let app = FissionServerApp::new("Test")
        .jobs(ServerJobRegistry::new().register_job(LOOP_JOB, |_request, _ctx| Ok(())))
        .server_route_widget::<LoopState, _>("/", "Home", None, LoopPage);
    let renderer = ServerRenderer::new(app).with_render_pass_limit(1);

    let error = renderer.handle(ServerRequest::get("/")).unwrap_err();
    assert!(error.to_string().contains("exceeded render pass limit"));
}

fn response_header<'a>(response: &'a ServerResponse, name: &str) -> Option<&'a str> {
    response
        .headers
        .iter()
        .find(|(candidate, _)| candidate.eq_ignore_ascii_case(name))
        .map(|(_, value)| value.as_str())
}

fn temp_project_dir(name: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("{name}-{}-{nanos}", std::process::id()))
}

fn env_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}
