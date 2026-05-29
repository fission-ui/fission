use crate::{ProgressiveWorker, ServerRenderPolicy, WasmIsland, WebRoute, WebRouteMode};
use anyhow::Result;
use fission_core::{
    AppState, BuildCtx, Env, Node, RuntimeResourceDeclaration, RuntimeState, View, Widget,
};
use fission_layout::LayoutSize;
use fission_theme::Theme;
use std::path::Path;
use std::sync::Arc;

pub(crate) type RouteRenderer =
    dyn for<'a> Fn(&ServerRenderContext<'a>) -> Result<ServerRenderedNode> + Send + Sync + 'static;

#[derive(Debug)]
pub(crate) struct ServerRenderedNode {
    pub node: Node,
    pub resources: Vec<RuntimeResourceDeclaration>,
}

#[derive(Clone, Debug)]
pub struct ServerRenderContext<'a> {
    pub project_dir: &'a Path,
    pub route_path: &'a str,
    pub theme: &'a Theme,
    pub viewport_size: LayoutSize,
}

#[derive(Clone)]
pub(crate) struct ServerRouteEntry {
    pub route: WebRoute,
    pub render: Arc<RouteRenderer>,
}

#[derive(Clone)]
pub struct FissionServerApp {
    pub(crate) project_name: String,
    pub(crate) project_dir: std::path::PathBuf,
    pub(crate) theme: Theme,
    pub(crate) routes: Vec<ServerRouteEntry>,
}

impl FissionServerApp {
    pub fn new(project_name: impl Into<String>) -> Self {
        Self {
            project_name: project_name.into(),
            project_dir: std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from(".")),
            theme: Theme::default(),
            routes: Vec::new(),
        }
    }

    pub fn project_dir(mut self, project_dir: impl Into<std::path::PathBuf>) -> Self {
        self.project_dir = project_dir.into();
        self
    }

    pub fn theme(mut self, theme: Theme) -> Self {
        self.theme = theme;
        self
    }

    pub fn route_widget<S, W>(
        mut self,
        path: impl Into<String>,
        title: impl Into<String>,
        description: impl Into<Option<String>>,
        mode: WebRouteMode,
        widget: W,
    ) -> Self
    where
        S: AppState + Default + 'static,
        W: Widget<S> + Clone + Send + Sync + 'static,
    {
        let widget = Arc::new(widget);
        self.routes.push(ServerRouteEntry {
            route: WebRoute {
                path: normalize_server_path(&path.into()),
                title: title.into(),
                description: description.into(),
                mode,
                workers: Vec::new(),
                islands: Vec::new(),
            },
            render: Arc::new(move |ctx| render_widget_node::<S, W>(widget.as_ref().clone(), ctx)),
        });
        self
    }

    pub fn worker(mut self, path: &str, worker: ProgressiveWorker) -> Self {
        let path = normalize_server_path(path);
        if let Some(route) = self
            .routes
            .iter_mut()
            .find(|entry| entry.route.path == path)
        {
            route.route.workers.push(worker);
        }
        self
    }

    pub fn island(mut self, path: &str, island: WasmIsland) -> Self {
        let path = normalize_server_path(path);
        if let Some(route) = self
            .routes
            .iter_mut()
            .find(|entry| entry.route.path == path)
        {
            route.route.islands.push(island);
        }
        self
    }

    pub fn server_route_widget<S, W>(
        self,
        path: impl Into<String>,
        title: impl Into<String>,
        description: impl Into<Option<String>>,
        widget: W,
    ) -> Self
    where
        S: AppState + Default + 'static,
        W: Widget<S> + Clone + Send + Sync + 'static,
    {
        self.route_widget(
            path,
            title,
            description,
            WebRouteMode::Server(ServerRenderPolicy::default()),
            widget,
        )
    }

    pub fn routes(&self) -> Vec<WebRoute> {
        self.routes
            .iter()
            .map(|entry| entry.route.clone())
            .collect()
    }

    pub(crate) fn find_route(&self, path: &str) -> Option<&ServerRouteEntry> {
        let path = normalize_server_path(path);
        self.routes.iter().find(|entry| entry.route.path == path)
    }
}

fn render_widget_node<S, W>(widget: W, ctx: &ServerRenderContext<'_>) -> Result<ServerRenderedNode>
where
    S: AppState + Default + 'static,
    W: Widget<S>,
{
    let runtime = RuntimeState::default();
    let mut env = Env::default();
    env.theme = ctx.theme.clone();
    env.viewport_size = ctx.viewport_size;
    let state = S::default();
    let view = View::new(&state, &runtime, &env, None);
    let mut build_ctx = BuildCtx::<S>::new();
    let node = widget.build(&mut build_ctx, &view);
    Ok(ServerRenderedNode {
        node,
        resources: build_ctx.resources.take(),
    })
}

pub(crate) fn normalize_server_path(path: &str) -> String {
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
