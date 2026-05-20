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
