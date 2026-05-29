use crate::app::{normalize_server_path, FissionServerApp, ServerRenderedNode, ServerRouteEntry};
use crate::{
    Cache, CacheEntry, CacheKey, CacheMetadata, CacheScope, Freshness, MokaCache, RenderedPage,
    WebRoute, WebRouteMode,
};
use anyhow::{anyhow, Result};
use fission_core::{Env, LoweringContext, RuntimeResourceDeclaration, RuntimeState};
use fission_layout::LayoutSize;
use fission_shell_site::{
    render_ir_to_html_with_styles, theme_variables_css, CssVariableMap, HtmlRenderOptions,
    StyleRegistry,
};
use serde::Serialize;
use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::SystemTime;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ServerRequest {
    pub method: String,
    pub path: String,
    pub query: BTreeMap<String, String>,
    pub headers: BTreeMap<String, String>,
}

impl ServerRequest {
    pub fn get(path: impl Into<String>) -> Self {
        Self {
            method: "GET".to_string(),
            path: path.into(),
            query: BTreeMap::new(),
            headers: BTreeMap::new(),
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
    viewport_size: LayoutSize,
}

impl ServerRenderer {
    pub fn new(app: FissionServerApp) -> Self {
        Self {
            app,
            cache: Arc::new(MokaCache::default()),
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

    pub fn routes(&self) -> Vec<WebRoute> {
        self.app.routes()
    }

    pub fn render_route(&self, path: &str) -> Result<RenderedServerRoute> {
        let route = self
            .app
            .find_route(path)
            .ok_or_else(|| anyhow!("server route `{}` was not found", path))?;
        self.render_uncached(route)
    }

    pub fn handle(&self, request: ServerRequest) -> Result<ServerResponse> {
        if request.method != "GET" {
            return Ok(ServerResponse::text(
                405,
                "text/plain; charset=utf-8",
                "method not allowed",
            ));
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
            let rendered = self.render_uncached(route)?;
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

        let rendered = self.render_uncached(route)?;
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

    fn render_uncached(&self, route: &ServerRouteEntry) -> Result<RenderedServerRoute> {
        let ctx = crate::ServerRenderContext {
            project_dir: &self.app.project_dir,
            route_path: &route.route.path,
            theme: &self.app.theme,
            viewport_size: self.viewport_size,
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
        let css = format!(
            "{}\n{}\n{}",
            theme_variables_css(":root", &self.app.theme),
            rendered.css,
            styles.to_css()
        );
        Ok(RenderedServerRoute {
            route: route.route.clone(),
            html: rendered.html,
            css,
            resources,
        })
    }
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
    use crate::{MokaCache, ProgressiveWorker, RevalidationPolicy, WasmIsland, WebRouteMode};
    use fission_core::ui::Text;
    use fission_core::{AppState, BuildCtx, Node, View, Widget};
    use std::sync::Arc;
    use std::time::Duration;

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
}
