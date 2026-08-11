use crate::app::{
    normalize_server_path, CacheInvalidationEndpoint, FissionServerApp, ServerEnvContext,
    ServerHttpContext, ServerRenderedNode, ServerRouteEntry, ServerRouteMatch, StaticMount,
};
use crate::{
    Cache, CacheEntry, CacheKey, CacheMetadata, CachePipeline, CacheScope, CacheTag, Freshness,
    InvalidationReport, MokaCache, RenderedPage, ServerActionSigner, ServerBrowserArtifactConfig,
    ServerCacheLayerConfig, ServerCacheProvider, ServerHttpConfig, ServerIslandConfig,
    ServerIslandPreload, ServerJobRegistry, ServerRuntimeConfig, ServerSameSite,
    ServerSessionConfig, SignedServerAction, VerifiedServerAction, WebRoute, WebRouteMode,
};
use anyhow::{anyhow, Context, Result};
use fission_core::internal::InternalLoweringCx;
use fission_core::ui::{Column, Overlay, ZStack};
use fission_core::{
    ActionEnvelope, ActionId, Env, RuntimeResourceDeclaration, RuntimeState, Widget,
};
use fission_ir::{semantics::ActionTrigger, CoreIR, Op};
use fission_layout::LayoutSize;
use fission_shell_site::{
    render_ir_to_html_with_styles, site_base_css, site_enhancement_js, theme_variables_css,
    CssVariableMap, HtmlRenderOptions, SitePageElement, SitePageElementPlacement, StyleRegistry,
};
use fission_theme::DesignMode;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicU16, AtomicU64, Ordering};
use std::sync::{Arc, RwLock};
use std::time::{Duration, SystemTime};

pub const MAX_SERVER_ACTION_BODY_BYTES: usize = 1024 * 1024;
pub const DEFAULT_SESSION_COOKIE_NAME: &str = "fission_session";
const SERVER_BROWSER_RUNTIME_JS: &str = include_str!("../assets/server-runtime.js");
static SESSION_COUNTER: AtomicU64 = AtomicU64::new(1);

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

    pub fn see_other(location: impl Into<String>) -> Self {
        Self {
            status: 303,
            headers: vec![
                ("location".to_string(), location.into()),
                ("cache-control".to_string(), "no-store".to_string()),
            ],
            body: Vec::new(),
            cache_status: None,
        }
    }

    pub fn body_string(&self) -> String {
        String::from_utf8_lossy(&self.body).into_owned()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ServerSession {
    id: String,
    is_new: bool,
}

impl ServerSession {
    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn is_new(&self) -> bool {
        self.is_new
    }
}

#[derive(Clone, Debug, Default, Deserialize)]
struct CacheInvalidationPayload {
    #[serde(default)]
    tags: Vec<String>,
    #[serde(default)]
    keys: Vec<String>,
}

#[derive(Clone, Debug)]
pub struct RenderedServerRoute {
    pub route: WebRoute,
    pub html: String,
    pub css: String,
    pub resources: Vec<RuntimeResourceDeclaration>,
    pub server_action_count: usize,
    pub status: u16,
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
    default_locale: String,
    http_config: ServerHttpConfig,
    session_config: ServerSessionConfig,
    session_signing_key: Option<[u8; 32]>,
    workers_config: ServerBrowserArtifactConfig,
    islands_config: ServerIslandConfig,
}

impl ServerRenderer {
    pub fn new(app: FissionServerApp) -> Self {
        let jobs = app.jobs.clone();
        let default_locale = app.default_locale.0.clone();
        Self {
            app,
            cache: Arc::new(MokaCache::default()),
            style_cache: RwLock::new(BTreeMap::new()),
            jobs,
            action_signer: ServerActionSigner::development(),
            allowed_action_origins: BTreeSet::new(),
            render_pass_limit: 4,
            viewport_size: LayoutSize::new(1280.0, 900.0),
            default_locale,
            http_config: ServerHttpConfig::default(),
            session_config: ServerSessionConfig::default(),
            session_signing_key: None,
            workers_config: ServerBrowserArtifactConfig::default(),
            islands_config: ServerIslandConfig::default(),
        }
    }

    pub fn configured(app: FissionServerApp) -> Result<Self> {
        let config = ServerRuntimeConfig::load(&app.project_dir)?;
        Self::with_config(app, config)
    }

    pub fn with_config(mut app: FissionServerApp, config: ServerRuntimeConfig) -> Result<Self> {
        if let Some(mode) = config.default_route_mode {
            app.apply_default_route_mode(mode);
        }
        let mut renderer = Self::new(app);
        if let Some(limit) = config.render_pass_limit {
            renderer = renderer.with_render_pass_limit(limit);
        }
        renderer.default_locale = config.default_locale;
        renderer.http_config = config.http;
        renderer.session_signing_key = session_signing_key(&config.sessions)?;
        renderer.session_config = config.sessions;
        renderer.workers_config = config.workers;
        renderer.islands_config = config.islands;
        renderer.validate_browser_artifact_config()?;
        renderer.cache = cache_from_config(&config.cache)?;
        Ok(renderer)
    }

    pub fn with_cache(mut self, cache: Arc<dyn Cache>) -> Self {
        self.cache = cache;
        self
    }

    pub fn cache(&self) -> Arc<dyn Cache> {
        self.cache.clone()
    }

    pub fn remove_cache_entry(&self, key: &CacheKey) -> Result<()> {
        self.cache.remove(key)?;
        Ok(())
    }

    pub fn remove_cache_entries<I, K>(&self, keys: I) -> Result<InvalidationReport>
    where
        I: IntoIterator<Item = K>,
        K: Into<String>,
    {
        let mut report = InvalidationReport::default();
        for key in keys {
            let key = CacheKey::new(key.into());
            if self.cache.get(&key)?.is_some() {
                report.removed_keys += 1;
                report.layers_affected = report.layers_affected.max(1);
            }
            self.cache.remove(&key)?;
        }
        Ok(report)
    }

    pub fn invalidate_cache_tag(&self, tag: impl Into<CacheTag>) -> Result<InvalidationReport> {
        Ok(self.cache.invalidate_tag(&tag.into())?)
    }

    pub fn invalidate_cache_tags<I, T>(&self, tags: I) -> Result<InvalidationReport>
    where
        I: IntoIterator<Item = T>,
        T: Into<CacheTag>,
    {
        let tags = tags.into_iter().map(Into::into).collect::<Vec<_>>();
        Ok(self.cache.invalidate_tags(&tags)?)
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
        let path = normalize_server_path(path);
        let route = self
            .app
            .find_route(&path)
            .ok_or_else(|| anyhow!("server route `{}` was not found", path))?;
        let request = ServerRequest::get(path);
        let session = self.session_for_request(&request)?;
        self.render_uncached(route, None, &request, &session)
    }

    pub fn handle(&self, request: ServerRequest) -> Result<ServerResponse> {
        if request.method == "POST" && normalize_server_path(&request.path) == "/__fission/action/"
        {
            return self.handle_action(request);
        }
        if request.method == "POST" {
            if let Some(endpoint) = self.app.find_cache_invalidation_endpoint(&request.path) {
                return self.handle_cache_invalidation(endpoint, &request);
            }
        }
        if let Some(handler) = self.app.find_http_handler(&request.method, &request.path) {
            let session = self.session_for_request(&request)?;
            let ctx = ServerHttpContext {
                project_dir: &self.app.project_dir,
                request: &request,
                session: &session,
            };
            let response = if tokio::runtime::Handle::try_current().is_ok() {
                std::thread::scope(|scope| {
                    scope
                        .spawn(|| (handler.handler)(&ctx))
                        .join()
                        .map_err(|_| anyhow!("server HTTP handler panicked"))?
                })
            } else {
                (handler.handler)(&ctx)
            };
            let mut response = response?;
            self.attach_session_cookie(&mut response, &session);
            return Ok(response);
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
        if let Some(response) =
            self.handle_static_mount_request(&asset_request_path(&request.path))?
        {
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
        let session = self.session_for_request(&request)?;

        if let WebRouteMode::Revalidated(policy) = &route.route.mode {
            let route_path = matched_route(route, &request).path;
            let env = self.env_for_route(route, None, &request, &session)?;
            let cache_key = self.cache_key_for_route(&route.route, &request, &env);
            let now = SystemTime::now();
            if let Some(entry) = self.cache.get(&cache_key)? {
                match entry.freshness(now) {
                    Freshness::Fresh | Freshness::Stale => {
                        if let Some(page) = entry.rendered_page() {
                            self.remember_route_css(&route_path, &page.css)?;
                            let mut response = page_response(page, entry.freshness(now));
                            response.headers.push((
                                "x-fission-cache".to_string(),
                                format!("{:?}", entry.freshness(now)).to_ascii_lowercase(),
                            ));
                            self.attach_session_cookie(&mut response, &session);
                            return Ok(response);
                        }
                    }
                    Freshness::Expired => {}
                }
            }
            let rendered = self.render_uncached_with_env(route, None, &request, &session, env)?;
            if rendered.server_action_count > 0 {
                anyhow::bail!(
                    "revalidated route `{}` renders server action forms; use ServerPrivate/Server mode or move the interactive region into an island before caching the page",
                    route.route.path
                );
            }
            let page = RenderedPage {
                html: rendered.html.clone(),
                css: rendered.css.clone(),
                status: rendered.status,
            };
            let entry = CacheEntry::full_page(
                cache_key,
                page.clone(),
                CacheScope::Public,
                policy.ttl,
                policy.stale_while_revalidate,
                policy.tags.clone(),
                CacheMetadata::full_page(&route_path),
            );
            self.cache.put(entry)?;
            let mut response = page_response(&page, Freshness::Expired);
            self.attach_session_cookie(&mut response, &session);
            return Ok(response);
        }

        let rendered = self.render_uncached(route, None, &request, &session)?;
        let mut response = ServerResponse {
            status: rendered.status,
            headers: vec![(
                "content-type".to_string(),
                "text/html; charset=utf-8".to_string(),
            )],
            body: rendered.html.into_bytes(),
            cache_status: None,
        };
        self.attach_session_cookie(&mut response, &session);
        Ok(response)
    }

    fn handle_cache_invalidation(
        &self,
        endpoint: &CacheInvalidationEndpoint,
        request: &ServerRequest,
    ) -> Result<ServerResponse> {
        let expected = format!("Bearer {}", endpoint.bearer_token);
        if header_value(&request.headers, "authorization").map(String::as_str) != Some(&expected) {
            return Ok(ServerResponse::text(
                401,
                "application/json; charset=utf-8",
                r#"{"error":"unauthorized"}"#,
            ));
        }
        let payload: CacheInvalidationPayload = serde_json::from_slice(&request.body)
            .map_err(|error| anyhow!("invalid cache invalidation request body: {error}"))?;
        if payload.tags.is_empty() && payload.keys.is_empty() {
            return Ok(ServerResponse::text(
                400,
                "application/json; charset=utf-8",
                r#"{"error":"no tags or keys provided"}"#,
            ));
        }

        let mut report = self.invalidate_cache_tags(payload.tags)?;
        let key_report = self.remove_cache_entries(payload.keys)?;
        report.removed_keys += key_report.removed_keys;
        report.removed_tags += key_report.removed_tags;
        report.layers_affected += key_report.layers_affected;
        Ok(ServerResponse::text(
            200,
            "application/json; charset=utf-8",
            serde_json::to_vec(&report)?,
        ))
    }

    fn render_uncached(
        &self,
        route: &ServerRouteEntry,
        action: Option<&VerifiedServerAction>,
        request: &ServerRequest,
        session: &ServerSession,
    ) -> Result<RenderedServerRoute> {
        let env = self.env_for_route(route, action, request, session)?;
        self.render_uncached_with_env(route, action, request, session, env)
    }

    fn render_uncached_with_env(
        &self,
        route: &ServerRouteEntry,
        action: Option<&VerifiedServerAction>,
        request: &ServerRequest,
        session: &ServerSession,
        env: Env,
    ) -> Result<RenderedServerRoute> {
        let route_match = matched_route(route, request);
        let route_path = route_match.path;
        let response_status = AtomicU16::new(200);
        let ctx = crate::ServerRenderContext {
            project_dir: &self.app.project_dir,
            route_path: &route_path,
            theme: &env.theme,
            viewport_size: self.viewport_size,
            jobs: &self.jobs,
            request,
            session,
            action,
            render_pass_limit: self.render_pass_limit,
            default_locale: &self.default_locale,
            route_params: route_match.params,
            env: &env,
            response_status: &response_status,
        };
        let mut rendered = if tokio::runtime::Handle::try_current().is_ok() {
            std::thread::scope(|scope| {
                scope
                    .spawn(|| (route.render)(&ctx))
                    .join()
                    .map_err(|_| anyhow!("server route renderer panicked"))?
            })
        } else {
            (route.render)(&ctx)
        }?;
        if let Some(render_footer) = &self.app.footer {
            let footer = if tokio::runtime::Handle::try_current().is_ok() {
                std::thread::scope(|scope| {
                    scope
                        .spawn(|| render_footer(&ctx))
                        .join()
                        .map_err(|_| anyhow!("server footer renderer panicked"))?
                })
            } else {
                render_footer(&ctx)
            }?;
            rendered.node = append_server_footer(rendered.node, footer.node);
            rendered.resources.extend(footer.resources);
            rendered
                .motion_declarations
                .extend(footer.motion_declarations);
            rendered
                .video_registrations
                .extend(footer.video_registrations);
            rendered.web_registrations.extend(footer.web_registrations);
            rendered.portals.extend(footer.portals);
        }
        let ServerRenderedNode {
            mut node,
            resources,
            motion_declarations,
            video_registrations,
            web_registrations,
            portals,
        } = rendered;
        node = compose_server_portals(node, portals);
        let runtime = RuntimeState::default();
        let mut lowering = InternalLoweringCx::new(&env, &runtime, None, None);
        let root = fission_core::internal::lower_widget(&node, &mut lowering);
        lowering.ir.set_root(root);

        let mut styles = StyleRegistry::default();
        let head_start_html = server_page_elements_for_route(
            self.app.document.page_elements(),
            &route_path,
            SitePageElementPlacement::HeadStart,
        );
        let mut head_end_html = server_page_elements_for_route(
            self.app.document.page_elements(),
            &route_path,
            SitePageElementPlacement::HeadEnd,
        );
        if matches!(self.islands_config.preload, ServerIslandPreload::Route) {
            head_end_html.extend(browser_artifact_preload_links(&route.route));
        }
        let body_start_html = server_page_elements_for_route(
            self.app.document.page_elements(),
            &route_path,
            SitePageElementPlacement::BodyStart,
        );
        let mut body_end_html = server_page_elements_for_route(
            self.app.document.page_elements(),
            &route_path,
            SitePageElementPlacement::BodyEnd,
        );
        if !route.route.workers.is_empty() || !route.route.islands.is_empty() {
            body_end_html.push(route_manifest_script(&route.route)?);
            body_end_html.push(server_browser_runtime_script());
        }
        let action_tokens = collect_server_action_tokens(
            &lowering.ir,
            &route_path,
            &self.action_signer,
            Duration::from_secs(10 * 60),
        )?;
        let server_action_count = action_tokens.len();
        let document_metadata = if let Some(resolver) = &self.app.document_metadata_resolver {
            resolver(&ctx, &route.route)?
        } else {
            crate::ServerDocumentMetadata::new(
                route.route.title.clone(),
                route.route.description.clone(),
            )
        };
        let render_options = HtmlRenderOptions {
            lang: env.locale.0.clone(),
            document_title: document_metadata.title,
            description: document_metadata.description,
            canonical_url: self.canonical_url_for_route(&route_path, request),
            site_name: Some(self.app.project_name.clone()),
            favicon_href: self.app.document.favicon_href().map(str::to_string),
            stylesheet_href: "/site.css".to_string(),
            current_route_path: route_path.clone(),
            css_variables: CssVariableMap::from_theme(&env.theme),
            default_theme_mode: self.app.document.default_theme_mode(),
            theme_switching: self.app.document.theme_switching(),
            code_highlighting: self
                .app
                .document
                .code_highlighting()
                .cloned()
                .unwrap_or_default(),
            server_action_post_path: Some("/__fission/action".to_string()),
            server_action_tokens: action_tokens,
            structured_data: route.route.structured_data.clone(),
            motion_declarations,
            video_registrations: video_registrations
                .into_iter()
                .map(|registration| (registration.node_id, registration))
                .collect(),
            web_registrations: web_registrations
                .into_iter()
                .map(|registration| (registration.node_id, registration))
                .collect(),
            font_faces: self.app.document.font_faces(),
            head_start_html,
            head_end_html,
            body_start_html,
            body_end_html,
            ..Default::default()
        };
        let rendered = render_ir_to_html_with_styles(&lowering.ir, &render_options, &mut styles)?;
        let css = rendered.css.clone();
        self.remember_route_css(&route_path, &css)?;
        Ok(RenderedServerRoute {
            route: route.route.clone(),
            html: rendered.html,
            css,
            resources,
            server_action_count,
            status: response_status.load(Ordering::Relaxed),
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
            "/server-runtime.js" => Ok(Some(ServerResponse::text(
                200,
                "application/javascript; charset=utf-8",
                SERVER_BROWSER_RUNTIME_JS,
            ))),
            "/favicon.ico" => Ok(Some(self.favicon_response()?)),
            path if path.starts_with("/assets/") => Ok(Some(self.project_asset_response(path)?)),
            _ => Ok(None),
        }
    }

    fn site_css_response(&self) -> Result<ServerResponse> {
        let mut css = String::new();
        css.push_str(site_base_css());
        css.push_str(
            "\n.fission-browser-action{cursor:pointer;user-select:none;display:inline-flex;align-items:center;justify-content:center;}\n.fission-browser-action:focus-visible{outline:3px solid rgba(96,165,250,.85);outline-offset:3px;}\n",
        );
        css.push('\n');
        if self.app.document.theme_switching() {
            let default_selector = match self
                .app
                .document
                .default_theme_mode()
                .unwrap_or(DesignMode::Light)
            {
                DesignMode::Light => ":root,[data-theme=\"light\"]",
                DesignMode::Dark => ":root,[data-theme=\"dark\"]",
            };
            css.push_str(&theme_variables_css(
                default_selector,
                self.app.document.theme(),
            ));
            if let Some(light) = self.app.document.light_theme() {
                css.push_str(&theme_variables_css("[data-theme=\"light\"]", light));
            }
            if let Some(dark) = self.app.document.dark_theme() {
                css.push_str(&theme_variables_css("[data-theme=\"dark\"]", dark));
            }
        } else {
            css.push_str(&theme_variables_css(":root", self.app.document.theme()));
        }
        let styles = self
            .style_cache
            .read()
            .map_err(|_| anyhow!("server style cache lock poisoned"))?;
        for style in styles.values() {
            css.push('\n');
            css.push_str(style);
        }
        for user_css in self.app.document.user_css() {
            css.push('\n');
            css.push_str(user_css);
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
        let mut roots = Vec::new();
        if let Some(path) = std::env::var_os("FISSION_SERVER_ARTIFACTS") {
            roots.push(PathBuf::from(path));
        }
        roots.extend([
            self.app.project_dir.join("target/fission/server"),
            self.app.project_dir.clone(),
            self.app.project_dir.join("public"),
        ]);
        for root in roots {
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

    fn handle_static_mount_request(&self, request_path: &str) -> Result<Option<ServerResponse>> {
        for mount in &self.app.static_mounts {
            let Some(relative) = static_mount_relative_path(mount, request_path) else {
                continue;
            };
            let Some(relative) = relative else {
                return Ok(Some(ServerResponse::text(
                    400,
                    "text/plain; charset=utf-8",
                    "invalid static path",
                )));
            };

            let root = static_mount_root(&self.app.project_dir, mount);
            let mut candidate = if relative.as_os_str().is_empty() {
                match &mount.index_file {
                    Some(index_file) => root.join(index_file),
                    None => root.clone(),
                }
            } else {
                root.join(&relative)
            };

            if candidate.is_dir() {
                if let Some(index_file) = &mount.index_file {
                    candidate = candidate.join(index_file);
                }
            }

            if candidate.is_file() {
                return Ok(Some(file_response(&candidate)?));
            }

            if mount.fallback_to_index && !looks_like_static_file_request(&relative) {
                if let Some(index_file) = &mount.index_file {
                    let index = root.join(index_file);
                    if index.is_file() {
                        return Ok(Some(file_response(&index)?));
                    }
                }
            }

            return Ok(Some(ServerResponse::text(
                404,
                "text/plain; charset=utf-8",
                "static file not found",
            )));
        }

        Ok(None)
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
        let token: SignedServerAction = match self.decode_action_request(&request) {
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
        let session = self.session_for_request(&request)?;
        let wants_redirect = action_request_should_redirect(&request);
        let rendered = self.render_uncached(route, Some(&action), &request, &session)?;
        let mut response = if wants_redirect {
            ServerResponse::see_other(route_path)
        } else {
            ServerResponse {
                status: 200,
                headers: vec![(
                    "content-type".to_string(),
                    "text/html; charset=utf-8".to_string(),
                )],
                body: rendered.html.into_bytes(),
                cache_status: None,
            }
        };
        self.attach_session_cookie(&mut response, &session);
        Ok(response)
    }

    fn decode_action_request(&self, request: &ServerRequest) -> Result<SignedServerAction> {
        let content_type = header_value(&request.headers, "content-type")
            .map(|value| value.split(';').next().unwrap_or(value).trim())
            .unwrap_or("application/json");
        if content_type.eq_ignore_ascii_case("application/x-www-form-urlencoded") {
            let body = String::from_utf8_lossy(&request.body);
            let token = form_value(&body, "token")
                .ok_or_else(|| anyhow!("server action form is missing token"))?;
            return self.action_signer.decode(&token);
        }
        serde_json::from_slice(&request.body).map_err(Into::into)
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

    fn session_for_request(&self, request: &ServerRequest) -> Result<ServerSession> {
        if let Some(cookie) = header_value(&request.headers, "cookie") {
            if let Some(value) = cookie_value(cookie, &self.session_config.cookie_name) {
                if let Some(id) = self.verify_session_cookie_value(&value) {
                    return Ok(ServerSession { id, is_new: false });
                }
            }
        }
        Ok(ServerSession {
            id: generate_session_id()?,
            is_new: true,
        })
    }

    fn attach_session_cookie(&self, response: &mut ServerResponse, session: &ServerSession) {
        if session.is_new {
            let secure = if self.session_config.secure {
                "; Secure"
            } else {
                ""
            };
            let same_site = match self.session_config.same_site {
                ServerSameSite::Strict => "Strict",
                ServerSameSite::Lax => "Lax",
                ServerSameSite::None => "None",
            };
            response.headers.push((
                "set-cookie".to_string(),
                format!(
                    "{}={}; Path=/; HttpOnly; SameSite={same_site}; Max-Age=2592000{secure}",
                    self.session_config.cookie_name,
                    self.encode_session_cookie_value(session.id())
                ),
            ));
        }
    }

    fn verify_session_cookie_value(&self, value: &str) -> Option<String> {
        match self.session_signing_key {
            Some(key) => {
                let (id, signature) = value.split_once('.')?;
                if safe_session_id(id)
                    && constant_time_eq(
                        session_signature(&key, &self.session_config.cookie_name, id).as_bytes(),
                        signature.as_bytes(),
                    )
                {
                    Some(id.to_string())
                } else {
                    None
                }
            }
            None => safe_session_id(value).then(|| value.to_string()),
        }
    }

    fn encode_session_cookie_value(&self, id: &str) -> String {
        match self.session_signing_key {
            Some(key) => format!(
                "{}.{}",
                id,
                session_signature(&key, &self.session_config.cookie_name, id)
            ),
            None => id.to_string(),
        }
    }

    fn canonical_url_for_route(&self, route_path: &str, request: &ServerRequest) -> Option<String> {
        let base = self.http_config.base_url.clone().or_else(|| {
            self.http_config
                .trust_proxy_headers
                .then(|| trusted_proxy_base_url(request))
                .flatten()
        });
        base.as_ref().map(|base| {
            if route_path == "/" {
                format!("{base}/")
            } else {
                format!("{base}{}", route_path.trim_end_matches('/'))
            }
        })
    }

    fn validate_browser_artifact_config(&self) -> Result<()> {
        if !self.workers_config.separate_artifacts {
            anyhow::bail!(
                "[server.workers].separate_artifacts = false is not supported; server workers are compiled as route-local artifacts"
            );
        }
        if !self.islands_config.separate_artifacts {
            anyhow::bail!(
                "[server.islands].separate_artifacts = false is not supported; server islands are compiled as route-local artifacts"
            );
        }
        Ok(())
    }

    fn env_for_route(
        &self,
        route: &ServerRouteEntry,
        action: Option<&VerifiedServerAction>,
        request: &ServerRequest,
        session: &ServerSession,
    ) -> Result<Env> {
        let route_match = matched_route(route, request);
        let ctx = ServerEnvContext {
            project_dir: &self.app.project_dir,
            route_path: &route_match.path,
            theme: self.app.document.theme(),
            viewport_size: self.viewport_size,
            jobs: &self.jobs,
            request,
            session,
            action,
            render_pass_limit: self.render_pass_limit,
            default_locale: &self.default_locale,
            route_params: route_match.params,
        };
        self.app.env_for_context(&ctx)
    }

    fn cache_key_for_route(
        &self,
        route: &WebRoute,
        request: &ServerRequest,
        env: &Env,
    ) -> CacheKey {
        let query = request
            .query
            .iter()
            .map(|(key, value)| format!("{key}={value}"))
            .collect::<Vec<_>>()
            .join("&");
        let vary = route
            .mode
            .revalidation()
            .map(|policy| {
                policy
                    .vary
                    .iter()
                    .map(|name| {
                        let normalized = name.trim().to_ascii_lowercase();
                        let value = header_value(&request.headers, &normalized)
                            .map(String::as_str)
                            .unwrap_or("");
                        format!("{normalized}={value}")
                    })
                    .collect::<Vec<_>>()
                    .join("&")
            })
            .unwrap_or_default();
        let theme_hash = blake3::hash(format!("{:?}", env.theme).as_bytes());
        let build_id = cache_build_id();
        let mut key = format!(
            "page:{}?{}#app:{}#locale:{}#theme:{}#build:{}",
            normalize_server_path(&request.path),
            query,
            self.app.project_name,
            env.locale.0,
            &theme_hash.to_hex().to_string()[..16],
            build_id
        );
        if !vary.is_empty() {
            key.push_str("#vary:");
            key.push_str(&vary);
        }
        CacheKey::new(key)
    }
}

fn server_page_elements_for_route(
    elements: &[SitePageElement],
    route_path: &str,
    placement: SitePageElementPlacement,
) -> Vec<String> {
    elements
        .iter()
        .filter(|element| element.placement == placement && element.applies_to(route_path))
        .map(|element| element.html.clone())
        .collect()
}

fn cache_build_id() -> String {
    cache_build_id_from(
        std::env::var("FISSION_BUILD_ID").ok(),
        option_env!("FISSION_BUILD_ID"),
        env!("CARGO_PKG_VERSION"),
    )
}

fn cache_build_id_from(
    runtime_build_id: Option<String>,
    compile_time_build_id: Option<&'static str>,
    package_version: &'static str,
) -> String {
    runtime_build_id
        .and_then(|value| {
            let value = value.trim();
            if value.is_empty() {
                None
            } else {
                Some(value.to_string())
            }
        })
        .or_else(|| {
            compile_time_build_id.and_then(|value| {
                let value = value.trim();
                if value.is_empty() {
                    None
                } else {
                    Some(value.to_string())
                }
            })
        })
        .unwrap_or_else(|| package_version.to_string())
}

fn matched_route(route: &ServerRouteEntry, request: &ServerRequest) -> ServerRouteMatch {
    let request_path = normalize_server_path(&request.path);
    route
        .match_request(&request_path)
        .unwrap_or_else(|| ServerRouteMatch {
            path: route.route.path.clone(),
            params: Default::default(),
        })
}

fn cache_from_config(config: &crate::ServerCacheConfig) -> Result<Arc<dyn Cache>> {
    match config.provider {
        ServerCacheProvider::Moka => Ok(Arc::new(MokaCache::new(config.moka.clone()))),
        ServerCacheProvider::Redis => redis_cache_from_config(config),
        ServerCacheProvider::Pipeline => {
            if config.layers.is_empty() {
                anyhow::bail!(
                    "[server.cache].provider = \"pipeline\" requires [[server.cache.layers]]"
                );
            }
            let mut layers = Vec::new();
            for layer in &config.layers {
                layers.push((cache_layer_from_config(layer)?, layer.policy));
            }
            Ok(Arc::new(CachePipeline::with_policies(layers)))
        }
    }
}

fn session_signing_key(config: &ServerSessionConfig) -> Result<Option<[u8; 32]>> {
    let Some(env) = &config.signing_key_env else {
        return Ok(None);
    };
    let secret = std::env::var(env)
        .with_context(|| format!("failed to read server session signing key from `{env}`"))?;
    if secret.trim().is_empty() {
        anyhow::bail!("server session signing key environment variable `{env}` is empty");
    }
    Ok(Some(*blake3::hash(secret.as_bytes()).as_bytes()))
}

fn cache_layer_from_config(config: &ServerCacheLayerConfig) -> Result<Arc<dyn Cache>> {
    match config.provider {
        ServerCacheProvider::Moka => Ok(Arc::new(MokaCache::new(config.moka.clone()))),
        ServerCacheProvider::Redis => redis_cache_from_layer_config(config),
        ServerCacheProvider::Pipeline => {
            anyhow::bail!("nested server cache pipelines are not supported")
        }
    }
}

#[cfg(feature = "redis")]
fn redis_cache_from_config(config: &crate::ServerCacheConfig) -> Result<Arc<dyn Cache>> {
    let url = resolve_redis_url(config.redis_url.as_deref(), config.redis_url_env.as_deref())?;
    let prefix = config.redis_prefix.as_deref().unwrap_or("fission");
    Ok(Arc::new(crate::RedisCache::new(&url, prefix)?))
}

#[cfg(feature = "redis")]
fn redis_cache_from_layer_config(config: &ServerCacheLayerConfig) -> Result<Arc<dyn Cache>> {
    let url = resolve_redis_url(config.redis_url.as_deref(), config.redis_url_env.as_deref())?;
    let prefix = config
        .redis_prefix
        .as_deref()
        .unwrap_or(config.name.as_str());
    Ok(Arc::new(crate::RedisCache::new(&url, prefix)?))
}

#[cfg(feature = "redis")]
fn resolve_redis_url(url: Option<&str>, env: Option<&str>) -> Result<String> {
    if let Some(url) = url {
        return Ok(url.to_string());
    }
    if let Some(env) = env {
        let value = std::env::var(env)
            .with_context(|| format!("failed to read Redis URL environment variable `{env}`"))?;
        return Ok(value);
    }
    anyhow::bail!("[server.cache].redis_url or url_env is required when provider = \"redis\"")
}

#[cfg(not(feature = "redis"))]
fn redis_cache_from_config(_config: &crate::ServerCacheConfig) -> Result<Arc<dyn Cache>> {
    anyhow::bail!(
        "[server.cache].provider = \"redis\" requires enabling the fission-shell-server `redis` feature"
    )
}

#[cfg(not(feature = "redis"))]
fn redis_cache_from_layer_config(_config: &ServerCacheLayerConfig) -> Result<Arc<dyn Cache>> {
    anyhow::bail!(
        "[server.cache].provider = \"redis\" requires enabling the fission-shell-server `redis` feature"
    )
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

fn form_value(body: &str, key: &str) -> Option<String> {
    body.split('&').find_map(|field| {
        let (candidate, value) = field.split_once('=')?;
        (candidate == key).then(|| form_decode(value))
    })
}

fn form_decode(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            b'+' => {
                out.push(b' ');
                index += 1;
            }
            b'%' if index + 2 < bytes.len() => {
                let hi = hex_value(bytes[index + 1]);
                let lo = hex_value(bytes[index + 2]);
                if let (Some(hi), Some(lo)) = (hi, lo) {
                    out.push((hi << 4) | lo);
                    index += 3;
                } else {
                    out.push(bytes[index]);
                    index += 1;
                }
            }
            byte => {
                out.push(byte);
                index += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn cookie_value(cookie: &str, key: &str) -> Option<String> {
    cookie.split(';').find_map(|part| {
        let (candidate, value) = part.trim().split_once('=')?;
        (candidate == key).then(|| value.to_string())
    })
}

fn safe_session_id(id: &str) -> bool {
    id.len() == 64 && id.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn generate_session_id() -> Result<String> {
    let mut random = [0u8; 32];
    getrandom::getrandom(&mut random)
        .map_err(|error| anyhow!("failed to create session id: {error}"))?;
    let counter = SESSION_COUNTER
        .fetch_add(1, Ordering::Relaxed)
        .to_le_bytes();
    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos()
        .to_le_bytes();
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"fission.server.session.v1");
    hasher.update(&random);
    hasher.update(&counter);
    hasher.update(&now);
    Ok(hasher.finalize().to_hex().to_string())
}

fn session_signature(key: &[u8; 32], cookie_name: &str, id: &str) -> String {
    let mut hasher = blake3::Hasher::new_keyed(key);
    hasher.update(b"fission.server.session.cookie.v1");
    hasher.update(cookie_name.as_bytes());
    hasher.update(id.as_bytes());
    to_hex(hasher.finalize().as_bytes())
}

fn trusted_proxy_base_url(request: &ServerRequest) -> Option<String> {
    let host = forwarded_header_value(request, "x-forwarded-host")
        .or_else(|| header_value(&request.headers, "host").cloned())?;
    let proto = forwarded_header_value(request, "x-forwarded-proto").or_else(|| {
        header_value(&request.headers, "x-forwarded-ssl")
            .filter(|value| value.eq_ignore_ascii_case("on"))
            .map(|_| "https".to_string())
    })?;
    let proto = proto.trim().to_ascii_lowercase();
    if !matches!(proto.as_str(), "http" | "https") || !safe_forwarded_host(&host) {
        return None;
    }
    Some(format!("{proto}://{}", host.trim()))
}

fn forwarded_header_value(request: &ServerRequest, name: &str) -> Option<String> {
    header_value(&request.headers, name)
        .and_then(|value| value.split(',').next())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn safe_forwarded_host(host: &str) -> bool {
    let trimmed = host.trim();
    !trimmed.is_empty()
        && trimmed.len() <= 253
        && !trimmed.starts_with('.')
        && !trimmed.ends_with('.')
        && trimmed.bytes().all(|byte| {
            byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b':' | b'[' | b']')
        })
}

fn to_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0x0f) as usize] as char);
    }
    out
}

fn constant_time_eq(left: &[u8], right: &[u8]) -> bool {
    if left.len() != right.len() {
        return false;
    }
    left.iter()
        .zip(right)
        .fold(0u8, |acc, (a, b)| acc | (a ^ b))
        == 0
}

fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
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

fn static_mount_relative_path(mount: &StaticMount, request_path: &str) -> Option<Option<PathBuf>> {
    let prefix = mount.url_prefix.as_str();
    let relative = if request_path == prefix {
        ""
    } else if prefix == "/" {
        request_path.trim_start_matches('/')
    } else {
        request_path.strip_prefix(&format!("{prefix}/"))?
    };
    Some(safe_relative_static_path(relative))
}

fn safe_relative_static_path(relative: &str) -> Option<PathBuf> {
    let path = Path::new(relative);
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Normal(segment) => out.push(segment),
            _ => return None,
        }
    }
    Some(out)
}

fn static_mount_root(project_dir: &Path, mount: &StaticMount) -> PathBuf {
    if mount.directory.is_absolute() {
        mount.directory.clone()
    } else {
        project_dir.join(&mount.directory)
    }
}

fn looks_like_static_file_request(relative: &Path) -> bool {
    relative.extension().is_some()
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
        Some("html") | Some("htm") => "text/html; charset=utf-8",
        Some("css") => "text/css; charset=utf-8",
        Some("js") | Some("mjs") => "application/javascript; charset=utf-8",
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

fn action_request_should_redirect(request: &ServerRequest) -> bool {
    let content_type = header_value(&request.headers, "content-type")
        .map(|value| value.split(';').next().unwrap_or(value).trim())
        .unwrap_or("application/json");
    content_type.eq_ignore_ascii_case("application/x-www-form-urlencoded")
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

fn collect_server_action_tokens(
    ir: &CoreIR,
    route_path: &str,
    signer: &ServerActionSigner,
    ttl: Duration,
) -> Result<BTreeMap<(fission_ir::WidgetId, u128), String>> {
    let mut tokens = BTreeMap::new();
    for node in ir.nodes.values() {
        let Op::Semantics(semantics) = &node.op else {
            continue;
        };
        for entry in &semantics.actions.entries {
            if entry.trigger != ActionTrigger::Default {
                continue;
            }
            let Some(payload) = entry.payload_data.clone() else {
                continue;
            };
            let envelope = ActionEnvelope {
                id: ActionId::from_u128(entry.action_id),
                payload,
            };
            let token =
                signer.sign_envelope(route_path.to_string(), node.id.as_u128(), envelope, ttl);
            tokens.insert((node.id, entry.action_id), signer.encode(&token)?);
        }
    }
    Ok(tokens)
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

fn browser_artifact_preload_links(route: &WebRoute) -> Vec<String> {
    route
        .workers
        .iter()
        .map(|worker| worker.artifact.as_str())
        .chain(route.islands.iter().map(|island| island.artifact.as_str()))
        .map(|artifact| {
            format!(
                "<link rel=\"preload\" href=\"{}\" as=\"fetch\" type=\"application/wasm\" crossorigin>",
                html_escape_attr(artifact)
            )
        })
        .collect()
}

fn compose_server_portals(
    node: Widget,
    portals: Vec<(Option<fission_ir::WidgetId>, Widget)>,
) -> Widget {
    if portals.is_empty() {
        return node;
    }
    Overlay {
        id: None,
        content: node,
        overlay: ZStack {
            id: None,
            children: portals.into_iter().map(|(_, portal)| portal).collect(),
        }
        .into(),
    }
    .into()
}

fn append_server_footer(node: Widget, footer: Widget) -> Widget {
    Column {
        children: vec![node, footer],
        flex_grow: 1.0,
        ..Default::default()
    }
    .into()
}

fn server_browser_runtime_script() -> String {
    "<script defer src=\"/server-runtime.js\"></script>".to_string()
}

fn html_escape_attr(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

#[cfg(test)]
#[path = "render_tests.rs"]
mod tests;
