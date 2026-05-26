use crate::build::{build_site, check_site, list_site_routes, SiteBuildOptions};
use anyhow::{bail, Context, Result};
use fission_core::{AppState, BuildCtx, Env, Node, RuntimeState, View, Widget};
use fission_layout::LayoutSize;
use fission_theme::{DesignMode, Theme};
use std::fs;
use std::io::{self, BufRead, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::Arc;

pub type ContentTransform = dyn Fn(&str, &Path, &Path) -> Result<String> + Send + Sync + 'static;

type RouteRenderer = dyn for<'a> Fn(&SiteRenderContext<'a>) -> Result<Node> + Send + Sync + 'static;

/// Position where raw static-site page markup is inserted.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SitePageElementPlacement {
    /// Insert immediately after the opening `<head>` tag, before generated metadata.
    HeadStart,
    /// Insert near the end of `<head>`, after generated metadata and generated assets.
    HeadEnd,
    /// Insert immediately after the opening `<body>` tag, before the rendered Fission root.
    BodyStart,
    /// Insert near the end of `<body>`, after the rendered Fission root.
    BodyEnd,
}

impl SitePageElementPlacement {
    pub(crate) fn parse(value: &str) -> Result<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "head-start" | "head_start" | "head:start" => Ok(Self::HeadStart),
            "head-end" | "head_end" | "head:end" | "head" => Ok(Self::HeadEnd),
            "body-start" | "body_start" | "body:start" => Ok(Self::BodyStart),
            "body-end" | "body_end" | "body:end" | "body" => Ok(Self::BodyEnd),
            other => anyhow::bail!(
                "unsupported static site page element placement `{other}`; expected head-start, head-end, body-start, or body-end"
            ),
        }
    }
}

/// Route filter for a raw static-site page element.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SitePageElementFilter {
    /// Insert the element into every generated page.
    All,
    /// Insert the element only when the normalized route path exactly matches.
    Exact(String),
    /// Insert the element when the normalized route path starts with the prefix.
    Prefix(String),
}

impl SitePageElementFilter {
    /// Creates a filter that matches one normalized route path exactly.
    ///
    /// Use this for scripts or metadata that belong to a single custom page. The
    /// path is normalized in the same way as site routes, so `"product"` and
    /// `"/product/"` both match the generated `/product/` route.
    pub fn exact(path: impl Into<String>) -> Self {
        Self::Exact(normalize_site_path(&path.into()))
    }

    /// Creates a filter that matches every route below a normalized prefix.
    ///
    /// Use this for page families such as `/docs/` or `/reference/`. Prefix
    /// matching is path-based and does not inspect page titles, source files, or
    /// Markdown front matter.
    pub fn prefix(path: impl Into<String>) -> Self {
        Self::Prefix(normalize_site_path(&path.into()))
    }

    pub(crate) fn matches(&self, route_path: &str) -> bool {
        let route_path = normalize_site_path(route_path);
        match self {
            Self::All => true,
            Self::Exact(path) => route_path == *path,
            Self::Prefix(prefix) => route_path.starts_with(prefix),
        }
    }
}

/// Raw static-site markup inserted into generated pages.
///
/// This is the escape hatch for host-owned page concerns such as analytics,
/// consent managers, verification tags, script preloads, or product-specific
/// metadata. It deliberately operates at the static-site shell boundary rather
/// than inside widgets: widgets still render Fission nodes, while page elements
/// describe document-level HTML that does not have a Fission widget equivalent.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SitePageElement {
    /// Where the HTML fragment is inserted in the generated document.
    pub placement: SitePageElementPlacement,
    /// Raw HTML to insert. The static-site shell does not escape this value.
    pub html: String,
    /// Optional route filters. An empty list is treated the same as `All`.
    pub filters: Vec<SitePageElementFilter>,
}

impl SitePageElement {
    /// Creates an element inserted near the end of `<head>` on every page.
    ///
    /// This is the usual placement for analytics snippets, verification tags,
    /// preload links, and other head-level metadata that must run or load after
    /// generated SEO tags are present.
    pub fn head(html: impl Into<String>) -> Self {
        Self::new(SitePageElementPlacement::HeadEnd, html)
    }

    /// Creates an element inserted near the end of `<body>` on every page.
    ///
    /// Use this for scripts that should run after the rendered Fission page root
    /// exists in the document. Prefer `defer` or event-driven scripts for
    /// anything that fetches network resources.
    pub fn body_end(html: impl Into<String>) -> Self {
        Self::new(SitePageElementPlacement::BodyEnd, html)
    }

    /// Creates a page element for a specific document placement.
    ///
    /// The returned element applies to every route until `only_route`,
    /// `route_prefix`, or `filter` is used. The `html` string is inserted as raw
    /// trusted markup; do not pass untrusted user content.
    pub fn new(placement: SitePageElementPlacement, html: impl Into<String>) -> Self {
        Self {
            placement,
            html: html.into(),
            filters: Vec::new(),
        }
    }

    /// Restricts this element to one exact generated route.
    ///
    /// Use this when a script, metadata block, or verification tag belongs only
    /// to one page. Calling this more than once adds another allowed route.
    pub fn only_route(mut self, path: impl Into<String>) -> Self {
        self.filters.push(SitePageElementFilter::exact(path));
        self
    }

    /// Restricts this element to all routes under one route prefix.
    ///
    /// Use this when a page family needs a shared script or document fragment,
    /// such as docs-only analytics, reference-only structured metadata, or a
    /// product-section experiment.
    pub fn route_prefix(mut self, path: impl Into<String>) -> Self {
        self.filters.push(SitePageElementFilter::prefix(path));
        self
    }

    /// Adds a custom route filter.
    ///
    /// This is useful when the caller already constructed filters from config.
    /// Multiple filters are ORed together; the element is emitted when any
    /// filter matches the current route.
    pub fn filter(mut self, filter: SitePageElementFilter) -> Self {
        self.filters.push(filter);
        self
    }

    pub(crate) fn applies_to(&self, route_path: &str) -> bool {
        self.filters.is_empty() || self.filters.iter().any(|filter| filter.matches(route_path))
    }
}

#[derive(Clone)]
pub struct CustomRoute {
    pub path: String,
    pub title: String,
    pub description: Option<String>,
    pub(crate) render: Arc<RouteRenderer>,
}

#[derive(Clone, Debug)]
pub struct SiteRenderContext<'a> {
    pub project_dir: &'a Path,
    pub route_path: &'a str,
    pub theme: &'a Theme,
}

#[derive(Clone)]
pub struct FissionSite {
    pub(crate) custom_routes: Vec<CustomRoute>,
    pub(crate) content_transform: Option<Arc<ContentTransform>>,
    pub(crate) theme: Theme,
    pub(crate) light_theme: Option<Theme>,
    pub(crate) dark_theme: Option<Theme>,
    pub(crate) default_theme_mode: Option<DesignMode>,
    pub(crate) theme_switching: bool,
    pub(crate) user_css: Vec<String>,
    pub(crate) footer: Option<Arc<RouteRenderer>>,
    pub(crate) page_elements: Vec<SitePageElement>,
}

impl Default for FissionSite {
    fn default() -> Self {
        Self {
            custom_routes: Vec::new(),
            content_transform: None,
            theme: Theme::default(),
            light_theme: None,
            dark_theme: None,
            default_theme_mode: None,
            theme_switching: false,
            user_css: Vec::new(),
            footer: None,
            page_elements: Vec::new(),
        }
    }
}

impl FissionSite {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn theme(mut self, theme: Theme) -> Self {
        self.theme = theme;
        self
    }

    pub fn light_dark_themes(
        mut self,
        light: Theme,
        dark: Theme,
        default_mode: DesignMode,
    ) -> Self {
        self.theme = match default_mode {
            DesignMode::Light => light.clone(),
            DesignMode::Dark => dark.clone(),
        };
        self.light_theme = Some(light);
        self.dark_theme = Some(dark);
        self.default_theme_mode = Some(default_mode);
        self.theme_switching = true;
        self
    }

    pub fn user_css(mut self, css: impl Into<String>) -> Self {
        self.user_css.push(css.into());
        self
    }

    /// Adds a raw document-level element to generated pages.
    ///
    /// Use this for page concerns that cannot be expressed as normal Fission
    /// widgets, such as analytics scripts, verification tags, consent-manager
    /// bootstraps, or provider-specific `<meta>` tags. The element can target
    /// every page or a filtered route set. The HTML is trusted raw markup and is
    /// not escaped by the renderer.
    pub fn page_element(mut self, element: SitePageElement) -> Self {
        self.page_elements.push(element);
        self
    }

    /// Adds raw markup near the end of `<head>` for every generated page.
    ///
    /// This is a convenience wrapper around `page_element(SitePageElement::head(...))`.
    /// Use `page_element` directly when the markup should only apply to selected
    /// routes.
    pub fn head_html(self, html: impl Into<String>) -> Self {
        self.page_element(SitePageElement::head(html))
    }

    /// Adds raw markup near the end of `<body>` for every generated page.
    ///
    /// This is a convenience wrapper around `page_element(SitePageElement::body_end(...))`.
    /// Use it for deferred scripts that need the rendered page root to already
    /// exist.
    pub fn body_end_html(self, html: impl Into<String>) -> Self {
        self.page_element(SitePageElement::body_end(html))
    }

    pub fn route_widget<S, W>(
        mut self,
        path: impl Into<String>,
        title: impl Into<String>,
        description: impl Into<Option<String>>,
        widget: W,
    ) -> Self
    where
        S: AppState + Default + 'static,
        W: Widget<S> + Clone + Send + Sync + 'static,
    {
        let widget = Arc::new(widget);
        self.custom_routes.push(CustomRoute {
            path: normalize_site_path(&path.into()),
            title: title.into(),
            description: description.into(),
            render: Arc::new(move |ctx| {
                let runtime = RuntimeState::default();
                let mut env = Env::default();
                env.theme = ctx.theme.clone();
                env.viewport_size = LayoutSize::new(1280.0, 900.0);
                let state = S::default();
                let view = View::new(&state, &runtime, &env, None);
                let mut build_ctx = BuildCtx::<S>::new();
                Ok(widget.as_ref().clone().build(&mut build_ctx, &view))
            }),
        });
        self
    }

    pub fn footer_widget<S, W>(mut self, widget: W) -> Self
    where
        S: AppState + Default + 'static,
        W: Widget<S> + Clone + Send + Sync + 'static,
    {
        let widget = Arc::new(widget);
        self.footer = Some(Arc::new(move |ctx| {
            render_widget_node::<S, W>(widget.as_ref().clone(), ctx)
        }));
        self
    }

    pub fn content_transform<F>(mut self, transform: F) -> Self
    where
        F: Fn(&str, &Path, &Path) -> Result<String> + Send + Sync + 'static,
    {
        self.content_transform = Some(Arc::new(transform));
        self
    }
}

fn render_widget_node<S, W>(widget: W, ctx: &SiteRenderContext<'_>) -> Result<Node>
where
    S: AppState + Default + 'static,
    W: Widget<S>,
{
    let runtime = RuntimeState::default();
    let mut env = Env::default();
    env.theme = ctx.theme.clone();
    env.viewport_size = LayoutSize::new(1280.0, 900.0);
    let state = S::default();
    let view = View::new(&state, &runtime, &env, None);
    let mut build_ctx = BuildCtx::<S>::new();
    Ok(widget.build(&mut build_ctx, &view))
}

pub fn build_from_cli(site: FissionSite) -> Result<()> {
    let args = SiteCliArgs::parse(std::env::args().skip(1))?;
    let options = SiteBuildOptions::from_project_dir(&args.project_dir, "Site")?;
    match args.command.as_str() {
        "build" => {
            let report = build_site(&options, &site)?;
            print_report("Built", &report);
        }
        "check" => {
            let report = check_site(&options, &site)?;
            print_report("Checked", &report);
        }
        "routes" => {
            for route in list_site_routes(&options, &site)? {
                println!(
                    "{}  {}  {}",
                    route.path,
                    route.title,
                    route.source.display()
                );
            }
        }
        "serve" => {
            let report = build_site(&options, &site)?;
            print_report("Built", &report);
            serve_static(options.output_dir, args.host, args.port, !args.no_open)?;
        }
        other => bail!("unknown site command `{other}`; expected build, check, routes, or serve"),
    }
    Ok(())
}

#[derive(Debug)]
struct SiteCliArgs {
    command: String,
    project_dir: PathBuf,
    host: String,
    port: u16,
    no_open: bool,
}

impl SiteCliArgs {
    fn parse<I>(args: I) -> Result<Self>
    where
        I: IntoIterator<Item = String>,
    {
        let mut command = None;
        let mut project_dir = PathBuf::from(".");
        let mut host = "127.0.0.1".to_string();
        let mut port = 8123u16;
        let mut no_open = false;
        let mut args = args.into_iter();
        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--project-dir" => {
                    project_dir = PathBuf::from(
                        args.next()
                            .context("--project-dir requires a project directory")?,
                    );
                }
                "--host" => {
                    host = args.next().context("--host requires a host value")?;
                }
                "--port" => {
                    port = args
                        .next()
                        .context("--port requires a port value")?
                        .parse()
                        .context("--port must be an integer")?;
                }
                "--no-open" => no_open = true,
                "--release" => {}
                value if value.starts_with('-') => bail!("unknown site flag `{value}`"),
                value => command = Some(value.to_string()),
            }
        }
        Ok(Self {
            command: command.unwrap_or_else(|| "build".to_string()),
            project_dir,
            host,
            port,
            no_open,
        })
    }
}

pub(crate) fn normalize_site_path(path: &str) -> String {
    let mut out = if path.starts_with('/') {
        path.to_string()
    } else {
        format!("/{path}")
    };
    while out.contains("//") {
        out = out.replace("//", "/");
    }
    if out.len() > 1 && !out.ends_with('/') {
        out.push('/');
    }
    out
}

fn print_report(label: &str, report: &crate::build::SiteBuildReport) {
    println!(
        "{} {} static route(s) into {}",
        label,
        report.routes.len(),
        report.output_dir.display()
    );
    for route in &report.routes {
        println!("{} -> {}", route.path, route.output.display());
    }
}

fn serve_static(root: PathBuf, host: String, port: u16, open: bool) -> Result<()> {
    let listener = TcpListener::bind((host.as_str(), port))
        .with_context(|| format!("failed to bind {}:{}", host, port))?;
    let url = format!("http://{host}:{port}/");
    println!("Serving {} at {}", root.display(), url);
    println!("Press Ctrl+C to stop.");
    if open {
        let _ = open_url(&url);
    }
    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                if let Err(error) = handle_http_request(stream, &root) {
                    eprintln!("request failed: {error}");
                }
            }
            Err(error) => eprintln!("accept failed: {error}"),
        }
    }
    Ok(())
}

fn handle_http_request(mut stream: TcpStream, root: &Path) -> Result<()> {
    let mut reader = io::BufReader::new(stream.try_clone()?);
    let mut request_line = String::new();
    reader.read_line(&mut request_line)?;
    let path = request_line
        .split_whitespace()
        .nth(1)
        .unwrap_or("/")
        .split('?')
        .next()
        .unwrap_or("/");
    let response = static_response(root, path)?;
    stream.write_all(&response)?;
    Ok(())
}

fn static_response(root: &Path, request_path: &str) -> Result<Vec<u8>> {
    let mut relative = request_path.trim_start_matches('/').to_string();
    if relative.is_empty() {
        relative = "index.html".to_string();
    }
    if relative.ends_with('/') {
        relative.push_str("index.html");
    }
    if !relative.ends_with(".html") && !relative.contains('.') {
        relative.push_str("/index.html");
    }
    let path = sanitize_static_path(root, &relative)?;
    if !path.exists() || !path.is_file() {
        return Ok(http_response(
            404,
            "text/plain; charset=utf-8",
            b"not found",
        ));
    }
    let body = fs::read(&path)?;
    Ok(http_response(200, content_type(&path), &body))
}

fn sanitize_static_path(root: &Path, relative: &str) -> Result<PathBuf> {
    let mut path = PathBuf::from(root);
    for part in relative.split('/') {
        if part.is_empty() || part == "." {
            continue;
        }
        if part == ".." || part.contains('\\') {
            bail!("invalid static path `{relative}`");
        }
        path.push(part);
    }
    Ok(path)
}

fn http_response(status: u16, content_type: &str, body: &[u8]) -> Vec<u8> {
    let reason = match status {
        200 => "OK",
        404 => "Not Found",
        _ => "Error",
    };
    let mut response = format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Length: {}\r\nContent-Type: {content_type}\r\nConnection: close\r\n\r\n",
        body.len()
    )
    .into_bytes();
    response.extend_from_slice(body);
    response
}

fn content_type(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("")
    {
        "html" => "text/html; charset=utf-8",
        "js" | "mjs" => "text/javascript; charset=utf-8",
        "wasm" => "application/wasm",
        "json" => "application/json; charset=utf-8",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "svg" => "image/svg+xml",
        "css" => "text/css; charset=utf-8",
        _ => "application/octet-stream",
    }
}

fn open_url(url: &str) -> Result<()> {
    let mut command = if cfg!(target_os = "macos") {
        let mut cmd = std::process::Command::new("open");
        cmd.arg(url);
        cmd
    } else if cfg!(target_os = "windows") {
        let mut cmd = std::process::Command::new("cmd");
        cmd.args(["/C", "start", "", url]);
        cmd
    } else {
        let mut cmd = std::process::Command::new("xdg-open");
        cmd.arg(url);
        cmd
    };
    command.spawn()?;
    Ok(())
}
