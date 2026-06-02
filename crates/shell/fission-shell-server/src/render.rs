use crate::app::{normalize_server_path, FissionServerApp, ServerRenderedNode, ServerRouteEntry};
use crate::{
    Cache, CacheEntry, CacheKey, CacheMetadata, CachePipeline, CacheScope, Freshness, MokaCache,
    RenderedPage, ServerActionSigner, ServerBrowserArtifactConfig, ServerCacheLayerConfig,
    ServerCacheProvider, ServerHttpConfig, ServerIslandConfig, ServerIslandPreload,
    ServerJobRegistry, ServerRuntimeConfig, ServerSameSite, ServerSessionConfig,
    SignedServerAction, VerifiedServerAction, WebRoute, WebRouteMode,
};
use anyhow::{anyhow, Context, Result};
use fission_core::internal::InternalLoweringCx;
use fission_core::{ActionEnvelope, ActionId, Env, RuntimeResourceDeclaration, RuntimeState};
use fission_ir::{semantics::ActionTrigger, CoreIR, Op};
use fission_layout::LayoutSize;
use fission_shell_site::{
    render_ir_to_html_with_styles, site_base_css, site_enhancement_js, theme_variables_css,
    CssVariableMap, HtmlRenderOptions, StyleRegistry,
};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
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

#[derive(Clone, Debug)]
pub struct RenderedServerRoute {
    pub route: WebRoute,
    pub html: String,
    pub css: String,
    pub resources: Vec<RuntimeResourceDeclaration>,
    pub server_action_count: usize,
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
        Self {
            app,
            cache: Arc::new(MokaCache::default()),
            style_cache: RwLock::new(BTreeMap::new()),
            jobs,
            action_signer: ServerActionSigner::development(),
            allowed_action_origins: BTreeSet::new(),
            render_pass_limit: 4,
            viewport_size: LayoutSize::new(1280.0, 900.0),
            default_locale: "en".to_string(),
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
        let request = ServerRequest::get(&route.route.path);
        let session = self.session_for_request(&request)?;
        self.render_uncached(route, None, &request, &session)
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
        let session = self.session_for_request(&request)?;

        if let WebRouteMode::Revalidated(policy) = &route.route.mode {
            let cache_key = self.cache_key_for_route(&route.route, &request);
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
                            self.attach_session_cookie(&mut response, &session);
                            return Ok(response);
                        }
                    }
                    Freshness::Expired => {}
                }
            }
            let rendered = self.render_uncached(route, None, &request, &session)?;
            if rendered.server_action_count > 0 {
                anyhow::bail!(
                    "revalidated route `{}` renders server action forms; use ServerPrivate/Server mode or move the interactive region into an island before caching the page",
                    route.route.path
                );
            }
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
            let mut response = page_response(&page, Freshness::Expired);
            self.attach_session_cookie(&mut response, &session);
            return Ok(response);
        }

        let rendered = self.render_uncached(route, None, &request, &session)?;
        let mut response = ServerResponse {
            status: 200,
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

    fn render_uncached(
        &self,
        route: &ServerRouteEntry,
        action: Option<&VerifiedServerAction>,
        request: &ServerRequest,
        session: &ServerSession,
    ) -> Result<RenderedServerRoute> {
        let ctx = crate::ServerRenderContext {
            project_dir: &self.app.project_dir,
            route_path: &route.route.path,
            theme: &self.app.theme,
            viewport_size: self.viewport_size,
            jobs: &self.jobs,
            request,
            session,
            action,
            render_pass_limit: self.render_pass_limit,
        };
        let ServerRenderedNode { node, resources } = (route.render)(&ctx)?;
        let runtime = RuntimeState::default();
        let mut env = Env::default();
        env.theme = self.app.theme.clone();
        env.viewport_size = self.viewport_size;
        let mut lowering = InternalLoweringCx::new(&env, &runtime, None, None);
        let root = fission_core::internal::lower_widget(&node, &mut lowering);
        lowering.ir.set_root(root);

        let mut styles = StyleRegistry::default();
        let mut head_end_html = Vec::new();
        if matches!(self.islands_config.preload, ServerIslandPreload::Route) {
            head_end_html.extend(browser_artifact_preload_links(&route.route));
        }
        let mut body_end_html = Vec::new();
        if !route.route.workers.is_empty() || !route.route.islands.is_empty() {
            body_end_html.push(route_manifest_script(&route.route)?);
            body_end_html.push(server_browser_runtime_script());
        }
        let action_tokens = collect_server_action_tokens(
            &lowering.ir,
            &route.route.path,
            &self.action_signer,
            Duration::from_secs(10 * 60),
        )?;
        let server_action_count = action_tokens.len();
        let render_options = HtmlRenderOptions {
            lang: self.default_locale.clone(),
            document_title: route.route.title.clone(),
            description: route.route.description.clone(),
            canonical_url: self.canonical_url_for_route(&route.route.path, request),
            site_name: Some(self.app.project_name.clone()),
            favicon_href: None,
            stylesheet_href: "/site.css".to_string(),
            current_route_path: route.route.path.clone(),
            css_variables: CssVariableMap::from_theme(&self.app.theme),
            server_action_post_path: Some("/__fission/action".to_string()),
            server_action_tokens: action_tokens,
            head_end_html,
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
            server_action_count,
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

    fn cache_key_for_route(&self, route: &WebRoute, request: &ServerRequest) -> CacheKey {
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
        let theme_hash = blake3::hash(format!("{:?}", self.app.theme).as_bytes());
        let build_id = option_env!("FISSION_BUILD_ID").unwrap_or(env!("CARGO_PKG_VERSION"));
        let mut key = format!(
            "page:{}?{}#app:{}#locale:{}#theme:{}#build:{}",
            route.path,
            query,
            self.app.project_name,
            self.default_locale,
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
mod tests {
    use super::*;
    use crate::{
        CacheError, CacheTag, InvalidationReport, MokaCache, ProgressiveWorker, RevalidationPolicy,
        WasmIsland, WebRouteMode,
    };
    use fission_core::ui::{Button, Text};
    use fission_core::{
        Action, ActionId, GlobalState, Handler, JobRef, JobResource, JobSpec, ReducerContext,
        ResourceKey, Widget,
    };
    use serde::{Deserialize, Serialize};
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex, OnceLock};
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    #[derive(Debug, Default)]
    struct TestState;
    impl GlobalState for TestState {}

    #[derive(Clone)]
    struct TestPage(&'static str);

    impl From<TestPage> for Widget {
        fn from(component: TestPage) -> Self {
            let (_ctx, _view) = fission_core::build::current::<TestState>();
            Text::new(component.0).into()
        }
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
        let key = renderer.cache_key_for_route(&renderer.routes()[0], &ServerRequest::get("/"));

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

        let first_key = renderer.cache_key_for_route(&renderer.routes()[0], &first);
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

        let en_key = renderer.cache_key_for_route(&renderer.routes()[0], &en);
        let fr_key = renderer.cache_key_for_route(&renderer.routes()[0], &fr);
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
}
