use crate::app::{normalize_server_path, FissionServerApp, ServerRenderedNode, ServerRouteEntry};
use crate::{
    Cache, CacheEntry, CacheKey, CacheMetadata, CacheScope, Freshness, MokaCache, RenderedPage,
    ServerActionSigner, ServerJobRegistry, SignedServerAction, VerifiedServerAction, WebRoute,
    WebRouteMode,
};
use anyhow::{anyhow, Result};
use fission_core::{Env, LoweringContext, RuntimeResourceDeclaration, RuntimeState};
use fission_layout::LayoutSize;
use fission_shell_site::{
    render_ir_to_html_with_styles, site_base_css, site_enhancement_js, theme_variables_css,
    CssVariableMap, HtmlRenderOptions, StyleRegistry,
};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::sync::{Arc, RwLock};
use std::time::SystemTime;

pub const MAX_SERVER_ACTION_BODY_BYTES: usize = 1024 * 1024;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ServerRequest {
    pub method: String,
    pub path: String,
    pub query: BTreeMap<String, String>,
    pub headers: BTreeMap<String, String>,
    pub body: Vec<u8>,
}

impl ServerRequest {
    pub fn get(path: impl Into<String>) -> Self {
        Self {
            method: "GET".to_string(),
            path: path.into(),
            query: BTreeMap::new(),
            headers: BTreeMap::new(),
            body: Vec::new(),
        }
    }

    pub fn post(path: impl Into<String>, body: impl Into<Vec<u8>>) -> Self {
        Self {
            method: "POST".to_string(),
            path: path.into(),
            query: BTreeMap::new(),
            headers: BTreeMap::new(),
            body: body.into(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ServerResponse {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
    pub cache_status: Option<Freshness>,
}

impl ServerResponse {
    pub fn text(status: u16, content_type: &str, body: impl Into<Vec<u8>>) -> Self {
        Self {
            status,
            headers: vec![("content-type".to_string(), content_type.to_string())],
            body: body.into(),
            cache_status: None,
        }
    }

    pub fn body_string(&self) -> String {
        String::from_utf8_lossy(&self.body).into_owned()
    }
}

#[derive(Clone, Debug)]
pub struct RenderedServerRoute {
    pub route: WebRoute,
    pub html: String,
    pub css: String,
    pub resources: Vec<RuntimeResourceDeclaration>,
}

pub struct ServerRenderer {
    app: FissionServerApp,
    cache: Arc<dyn Cache>,
    style_cache: RwLock<BTreeMap<String, String>>,
    jobs: ServerJobRegistry,
    action_signer: ServerActionSigner,
    allowed_action_origins: BTreeSet<String>,
    render_pass_limit: usize,
    viewport_size: LayoutSize,
}

impl ServerRenderer {
    pub fn new(app: FissionServerApp) -> Self {
        let jobs = app.jobs.clone();
        Self {
            app,
            cache: Arc::new(MokaCache::default()),
            style_cache: RwLock::new(BTreeMap::new()),
            jobs,
            action_signer: ServerActionSigner::development(),
            allowed_action_origins: BTreeSet::new(),
            render_pass_limit: 4,
            viewport_size: LayoutSize::new(1280.0, 900.0),
        }
    }

    pub fn with_cache(mut self, cache: Arc<dyn Cache>) -> Self {
        self.cache = cache;
        self
    }

    pub fn with_viewport_size(mut self, size: LayoutSize) -> Self {
        self.viewport_size = size;
        self
    }

    pub fn with_jobs(mut self, jobs: ServerJobRegistry) -> Self {
        self.jobs = jobs;
        self
    }

    pub fn with_action_signer(mut self, signer: ServerActionSigner) -> Self {
        self.action_signer = signer;
        self
    }

    pub fn with_allowed_action_origin(mut self, origin: impl Into<String>) -> Self {
        self.allowed_action_origins.insert(origin.into());
        self
    }

    pub fn with_allowed_action_origins<I, O>(mut self, origins: I) -> Self
    where
        I: IntoIterator<Item = O>,
        O: Into<String>,
    {
        self.allowed_action_origins
            .extend(origins.into_iter().map(Into::into));
        self
    }

    pub fn with_render_pass_limit(mut self, limit: usize) -> Self {
        self.render_pass_limit = limit;
        self
    }

    pub fn sign_action<A: fission_core::Action>(
        &self,
        route_path: impl Into<String>,
        target_node: u128,
        action: A,
        ttl: std::time::Duration,
    ) -> SignedServerAction {
        self.action_signer
            .sign(route_path, target_node, action, ttl)
    }

    pub fn routes(&self) -> Vec<WebRoute> {
        self.app.routes()
    }

    pub fn render_route(&self, path: &str) -> Result<RenderedServerRoute> {
        let route = self
            .app
            .find_route(path)
            .ok_or_else(|| anyhow!("server route `{}` was not found", path))?;
        self.render_uncached(route, None)
    }

    pub fn handle(&self, request: ServerRequest) -> Result<ServerResponse> {
        if request.method == "POST" && normalize_server_path(&request.path) == "/__fission/action/"
        {
            return self.handle_action(request);
        }
        if request.method != "GET" {
            return Ok(ServerResponse::text(
                405,
                "text/plain; charset=utf-8",
                "method not allowed",
            ));
        }
        if let Some(response) = self.handle_asset_request(&asset_request_path(&request.path))? {
            return Ok(response);
        }
        let path = normalize_server_path(&request.path);
        let Some(route) = self.app.find_route(&path) else {
            return Ok(ServerResponse::text(
                404,
                "text/plain; charset=utf-8",
                "not found",
            ));
        };

        if let WebRouteMode::Revalidated(policy) = &route.route.mode {
            let cache_key = cache_key_for_route(&route.route, &request);
            let now = SystemTime::now();
            if let Some(entry) = self.cache.get(&cache_key)? {
                match entry.freshness(now) {
                    Freshness::Fresh | Freshness::Stale => {
                        if let Some(page) = entry.rendered_page() {
                            self.remember_route_css(&route.route.path, &page.css)?;
                            let mut response = page_response(page, entry.freshness(now));
                            response.headers.push((
                                "x-fission-cache".to_string(),
                                format!("{:?}", entry.freshness(now)).to_ascii_lowercase(),
                            ));
                            return Ok(response);
                        }
                    }
                    Freshness::Expired => {}
                }
            }
            let rendered = self.render_uncached(route, None)?;
            let page = RenderedPage {
                html: rendered.html.clone(),
                css: rendered.css.clone(),
                status: 200,
            };
            let entry = CacheEntry::full_page(
                cache_key,
                page.clone(),
                CacheScope::Public,
                policy.ttl,
                policy.stale_while_revalidate,
                policy.tags.clone(),
                CacheMetadata::full_page(&route.route.path),
            );
            self.cache.put(entry)?;
            return Ok(page_response(&page, Freshness::Expired));
        }

        let rendered = self.render_uncached(route, None)?;
        Ok(ServerResponse {
            status: 200,
            headers: vec![(
                "content-type".to_string(),
                "text/html; charset=utf-8".to_string(),
            )],
            body: rendered.html.into_bytes(),
            cache_status: None,
        })
    }

    fn render_uncached(
        &self,
        route: &ServerRouteEntry,
        action: Option<&VerifiedServerAction>,
    ) -> Result<RenderedServerRoute> {
        let ctx = crate::ServerRenderContext {
            project_dir: &self.app.project_dir,
            route_path: &route.route.path,
            theme: &self.app.theme,
            viewport_size: self.viewport_size,
            jobs: &self.jobs,
            action,
            render_pass_limit: self.render_pass_limit,
        };
        let ServerRenderedNode { node, resources } = (route.render)(&ctx)?;
        let runtime = RuntimeState::default();
        let mut env = Env::default();
        env.theme = self.app.theme.clone();
        env.viewport_size = self.viewport_size;
        let mut lowering = LoweringContext::new(&env, &runtime, None, None);
        let root = node.lower(&mut lowering);
        lowering.ir.set_root(root);

        let mut styles = StyleRegistry::default();
        let mut body_end_html = Vec::new();
        if !route.route.workers.is_empty() || !route.route.islands.is_empty() {
            body_end_html.push(route_manifest_script(&route.route)?);
        }
        let render_options = HtmlRenderOptions {
            lang: "en".to_string(),
            document_title: route.route.title.clone(),
            description: route.route.description.clone(),
            canonical_url: None,
            site_name: Some(self.app.project_name.clone()),
            favicon_href: None,
            stylesheet_href: "/site.css".to_string(),
            current_route_path: route.route.path.clone(),
            css_variables: CssVariableMap::from_theme(&self.app.theme),
            body_end_html,
            ..Default::default()
        };
        let rendered = render_ir_to_html_with_styles(&lowering.ir, &render_options, &mut styles)?;
        let css = rendered.css.clone();
        self.remember_route_css(&route.route.path, &css)?;
        Ok(RenderedServerRoute {
            route: route.route.clone(),
            html: rendered.html,
            css,
            resources,
        })
    }

    fn handle_asset_request(&self, request_path: &str) -> Result<Option<ServerResponse>> {
        match request_path {
            "/site.css" => Ok(Some(self.site_css_response()?)),
            "/site-enhancement.js" => Ok(Some(ServerResponse::text(
                200,
                "application/javascript; charset=utf-8",
                site_enhancement_js(),
            ))),
            "/favicon.ico" => Ok(Some(self.favicon_response()?)),
            path if path.starts_with("/assets/") => Ok(Some(self.project_asset_response(path)?)),
            _ => Ok(None),
        }
    }

    fn site_css_response(&self) -> Result<ServerResponse> {
        let mut css = String::new();
        css.push_str(site_base_css());
        css.push('\n');
        css.push_str(&theme_variables_css(":root", &self.app.theme));
        let styles = self
            .style_cache
            .read()
            .map_err(|_| anyhow!("server style cache lock poisoned"))?;
        for style in styles.values() {
            css.push('\n');
            css.push_str(style);
        }
        Ok(ServerResponse::text(200, "text/css; charset=utf-8", css))
    }

    fn remember_route_css(&self, route_path: &str, css: &str) -> Result<()> {
        self.style_cache
            .write()
            .map_err(|_| anyhow!("server style cache lock poisoned"))?
            .insert(route_path.to_string(), css.to_string());
        Ok(())
    }

    fn favicon_response(&self) -> Result<ServerResponse> {
        for path in [
            self.app.project_dir.join("favicon.ico"),
            self.app.project_dir.join("assets/favicon.ico"),
            self.app.project_dir.join("public/favicon.ico"),
        ] {
            if path.is_file() {
                return file_response(&path);
            }
        }
        Ok(ServerResponse {
            status: 204,
            headers: vec![("content-type".to_string(), "image/x-icon".to_string())],
            body: Vec::new(),
            cache_status: None,
        })
    }

    fn project_asset_response(&self, request_path: &str) -> Result<ServerResponse> {
        let Some(relative) = safe_relative_asset_path(request_path) else {
            return Ok(ServerResponse::text(
                400,
                "text/plain; charset=utf-8",
                "invalid asset path",
            ));
        };
        for root in [
            self.app.project_dir.join("target/fission/server"),
            self.app.project_dir.clone(),
            self.app.project_dir.join("public"),
        ] {
            let candidate = root.join(&relative);
            if candidate.is_file() {
                return file_response(&candidate);
            }
        }
        Ok(ServerResponse::text(
            404,
            "text/plain; charset=utf-8",
            "asset not found",
        ))
    }

    fn handle_action(&self, request: ServerRequest) -> Result<ServerResponse> {
        if request.body.len() > MAX_SERVER_ACTION_BODY_BYTES {
            return Ok(ServerResponse::text(
                413,
                "text/plain; charset=utf-8",
                "server action body too large",
            ));
        }
        if !self.action_origin_allowed(&request) {
            return Ok(ServerResponse::text(
                403,
                "text/plain; charset=utf-8",
                "server action origin rejected",
            ));
        }
        let token: SignedServerAction = match serde_json::from_slice(&request.body) {
            Ok(token) => token,
            Err(_) => {
                return Ok(ServerResponse::text(
                    400,
                    "text/plain; charset=utf-8",
                    "invalid server action token",
                ))
            }
        };
        let action = match self.action_signer.verify_once(&token) {
            Ok(action) => action,
            Err(_) => {
                return Ok(ServerResponse::text(
                    403,
                    "text/plain; charset=utf-8",
                    "server action token rejected",
                ))
            }
        };
        let route_path = normalize_server_path(&action.route_path);
        let Some(route) = self.app.find_route(&route_path) else {
            return Ok(ServerResponse::text(
                404,
                "text/plain; charset=utf-8",
                "server action route not found",
            ));
        };
        let rendered = self.render_uncached(route, Some(&action))?;
        Ok(ServerResponse {
            status: 200,
            headers: vec![(
                "content-type".to_string(),
                "text/html; charset=utf-8".to_string(),
            )],
            body: rendered.html.into_bytes(),
            cache_status: None,
        })
    }

    fn action_origin_allowed(&self, request: &ServerRequest) -> bool {
        if self.allowed_action_origins.is_empty() {
            return true;
        }
        let Some(origin) = header_value(&request.headers, "origin") else {
            return true;
        };
        self.allowed_action_origins.contains(origin)
    }
}

fn asset_request_path(path: &str) -> String {
    let mut out = if path.starts_with('/') {
        path.to_string()
    } else {
        format!("/{path}")
    };
    while out.contains("//") {
        out = out.replace("//", "/");
    }
    if out.len() > 1 {
        out = out.trim_end_matches('/').to_string();
    }
    out
}

fn safe_relative_asset_path(request_path: &str) -> Option<PathBuf> {
    let path = Path::new(request_path.trim_start_matches('/'));
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Normal(segment) => out.push(segment),
            _ => return None,
        }
    }
    (!out.as_os_str().is_empty()).then_some(out)
}

fn file_response(path: &Path) -> Result<ServerResponse> {
    let body = fs::read(path)?;
    Ok(ServerResponse {
        status: 200,
        headers: vec![(
            "content-type".to_string(),
            content_type_for_path(path).to_string(),
        )],
        body,
        cache_status: None,
    })
}

fn content_type_for_path(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| extension.to_ascii_lowercase())
        .as_deref()
    {
        Some("css") => "text/css; charset=utf-8",
        Some("js") => "application/javascript; charset=utf-8",
        Some("wasm") => "application/wasm",
        Some("ico") => "image/x-icon",
        Some("svg") => "image/svg+xml",
        Some("png") => "image/png",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("webp") => "image/webp",
        Some("json") => "application/json; charset=utf-8",
        Some("txt") => "text/plain; charset=utf-8",
        _ => "application/octet-stream",
    }
}

fn header_value<'a>(headers: &'a BTreeMap<String, String>, name: &str) -> Option<&'a String> {
    headers
        .iter()
        .find(|(candidate, _)| candidate.eq_ignore_ascii_case(name))
        .map(|(_, value)| value)
}

fn page_response(page: &RenderedPage, freshness: Freshness) -> ServerResponse {
    ServerResponse {
        status: page.status,
        headers: vec![(
            "content-type".to_string(),
            "text/html; charset=utf-8".to_string(),
        )],
        body: page.html.clone().into_bytes(),
        cache_status: Some(freshness),
    }
}

fn cache_key_for_route(route: &WebRoute, request: &ServerRequest) -> CacheKey {
    let query = request
        .query
        .iter()
        .map(|(key, value)| format!("{key}={value}"))
        .collect::<Vec<_>>()
        .join("&");
    CacheKey::new(format!("page:{}?{}", route.path, query))
}

#[derive(Serialize)]
struct RouteManifest<'a> {
    route: &'a str,
    mode: &'a str,
    workers: &'a [crate::ProgressiveWorker],
    islands: &'a [crate::WasmIsland],
}

fn route_manifest_script(route: &WebRoute) -> Result<String> {
    let mode = match &route.mode {
        WebRouteMode::Static => "static",
        WebRouteMode::Revalidated(_) => "revalidated",
        WebRouteMode::Server(_) => "server",
        WebRouteMode::ServerPrivate(_) => "server_private",
        WebRouteMode::ClientApp(_) => "client_app",
    };
    let manifest = RouteManifest {
        route: &route.path,
        mode,
        workers: &route.workers,
        islands: &route.islands,
    };
    let json = serde_json::to_string(&manifest)?;
    Ok(format!(
        "<script type=\"application/json\" id=\"fission-route-manifest\">{json}</script>"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        CacheError, CacheTag, InvalidationReport, MokaCache, ProgressiveWorker, RevalidationPolicy,
        WasmIsland, WebRouteMode,
    };
    use fission_core::ui::Text;
    use fission_core::{
        Action, ActionId, AppState, BuildCtx, Handler, JobRef, JobResource, JobSpec, Node,
        ReducerContext, ResourceKey, View, Widget,
    };
    use serde::{Deserialize, Serialize};
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    #[derive(Debug, Default)]
    struct TestState;
    impl AppState for TestState {}

    #[derive(Clone)]
    struct TestPage(&'static str);

    impl Widget<TestState> for TestPage {
        fn build(&self, _ctx: &mut BuildCtx<TestState>, _view: &View<TestState>) -> Node {
            Text::new(self.0).into_node()
        }
    }

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
    struct TestAction;

    impl Action for TestAction {
        fn static_id() -> ActionId {
            ActionId::from_name("server-renderer.test-action")
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

        let first = renderer.handle(ServerRequest::get("/")).unwrap();
        assert_eq!(first.status, 200);
        assert!(first.body_string().contains("Hello cache"));

        let second = renderer.handle(ServerRequest::get("/")).unwrap();
        assert_eq!(second.cache_status, Some(Freshness::Fresh));
        assert!(cache
            .contains_fresh(&CacheKey::new("page:/?"), SystemTime::now())
            .unwrap());
    }

    #[test]
    fn revalidated_cache_key_normalizes_query_order() {
        let cache = Arc::new(MokaCache::default());
        let app = FissionServerApp::new("Test").route_widget::<TestState, _>(
            "/search",
            "Search",
            None,
            WebRouteMode::Revalidated(
                RevalidationPolicy::new(Duration::from_secs(60)).tag("search"),
            ),
            TestPage("Hello query cache"),
        );
        let renderer = ServerRenderer::new(app).with_cache(cache.clone());
        let mut first = ServerRequest::get("/search");
        first.query.insert("b".to_string(), "2".to_string());
        first.query.insert("a".to_string(), "1".to_string());
        let mut second = ServerRequest::get("/search");
        second.query.insert("a".to_string(), "1".to_string());
        second.query.insert("b".to_string(), "2".to_string());

        assert_eq!(
            renderer.handle(first).unwrap().cache_status,
            Some(Freshness::Expired)
        );
        assert_eq!(
            renderer.handle(second).unwrap().cache_status,
            Some(Freshness::Fresh)
        );
        assert!(cache
            .contains_fresh(&CacheKey::new("page:/search/?a=1&b=2"), SystemTime::now())
            .unwrap());
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
            WebRouteMode::Revalidated(
                RevalidationPolicy::new(Duration::from_millis(1)).tag("home"),
            ),
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

    impl Widget<TestState> for MissingJobPage {
        fn build(&self, ctx: &mut BuildCtx<TestState>, _view: &View<TestState>) -> Node {
            ctx.resources.job(JobResource::new(
                ResourceKey::new("missing-job"),
                MISSING_JOB,
                MissingJobRequest,
            ));
            Text::new("Missing job").into_node()
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

    impl AppState for LoopState {}

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

    impl Widget<LoopState> for LoopPage {
        fn build(&self, ctx: &mut BuildCtx<LoopState>, view: &View<LoopState>) -> Node {
            let on_ok = ctx.bind(LoopLoaded, on_loop_loaded as Handler<LoopState, LoopLoaded>);
            ctx.resources.job(
                JobResource::new(
                    ResourceKey::new("loop-job"),
                    LOOP_JOB,
                    LoopJobRequest {
                        count: view.state.count,
                    },
                )
                .deps(view.state.count)
                .on_ok(on_ok),
            );
            Text::new(format!("loop {}", view.state.count)).into_node()
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
}
