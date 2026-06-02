use crate::render::{ServerRequest, ServerSession};
use crate::{
    ProgressiveWorker, ServerJobRegistry, ServerRenderPolicy, VerifiedServerAction, WasmIsland,
    WebRoute, WebRouteMode,
};
use anyhow::Result;
use fission_core::internal::BuildCtx;
use fission_core::{
    ActionInput, Effect, Env, GlobalState, RuntimeResourceDeclaration, RuntimeResourceKind,
    RuntimeState, View, Widget, WidgetId,
};
use fission_layout::LayoutSize;
use fission_theme::Theme;
use std::collections::BTreeSet;
use std::path::Path;
use std::sync::Arc;

pub(crate) type RouteRenderer =
    dyn for<'a> Fn(&ServerRenderContext<'a>) -> Result<ServerRenderedNode> + Send + Sync + 'static;
type InitialStateLoader<S> =
    dyn for<'a> Fn(&ServerRenderContext<'a>) -> Result<S> + Send + Sync + 'static;

#[derive(Debug)]
pub(crate) struct ServerRenderedNode {
    pub node: Widget,
    pub resources: Vec<RuntimeResourceDeclaration>,
}

#[derive(Clone)]
pub struct ServerRenderContext<'a> {
    pub project_dir: &'a Path,
    pub route_path: &'a str,
    pub theme: &'a Theme,
    pub viewport_size: LayoutSize,
    pub jobs: &'a ServerJobRegistry,
    pub request: &'a ServerRequest,
    pub session: &'a ServerSession,
    pub action: Option<&'a VerifiedServerAction>,
    pub render_pass_limit: usize,
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
    pub(crate) jobs: ServerJobRegistry,
    pub(crate) routes: Vec<ServerRouteEntry>,
}

impl FissionServerApp {
    pub fn new(project_name: impl Into<String>) -> Self {
        Self {
            project_name: project_name.into(),
            project_dir: std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from(".")),
            theme: Theme::default(),
            jobs: ServerJobRegistry::new(),
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

    pub fn jobs(mut self, jobs: ServerJobRegistry) -> Self {
        self.jobs = jobs;
        self
    }

    pub fn route_widget<S, W>(
        self,
        path: impl Into<String>,
        title: impl Into<String>,
        description: impl Into<Option<String>>,
        mode: WebRouteMode,
        widget: W,
    ) -> Self
    where
        S: GlobalState + Default + 'static,
        W: Clone + Into<Widget> + Send + Sync + 'static,
    {
        self.route_widget_with_state(path, title, description, mode, widget, |_| Ok(S::default()))
    }

    pub fn route_widget_with_state<S, W, F>(
        mut self,
        path: impl Into<String>,
        title: impl Into<String>,
        description: impl Into<Option<String>>,
        mode: WebRouteMode,
        widget: W,
        initial_state: F,
    ) -> Self
    where
        S: GlobalState + 'static,
        W: Clone + Into<Widget> + Send + Sync + 'static,
        F: for<'a> Fn(&ServerRenderContext<'a>) -> Result<S> + Send + Sync + 'static,
    {
        let widget = Arc::new(widget);
        let initial_state: Arc<InitialStateLoader<S>> = Arc::new(initial_state);
        self.routes.push(ServerRouteEntry {
            route: WebRoute {
                path: normalize_server_path(&path.into()),
                title: title.into(),
                description: description.into(),
                mode,
                workers: Vec::new(),
                islands: Vec::new(),
            },
            render: Arc::new(move |ctx| {
                let state = initial_state(ctx)?;
                render_widget_node::<S, W>(widget.as_ref(), ctx, state)
            }),
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
        S: GlobalState + Default + 'static,
        W: Clone + Into<Widget> + Send + Sync + 'static,
    {
        self.route_widget::<S, W>(
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

    pub(crate) fn apply_default_route_mode(&mut self, mode: WebRouteMode) {
        for entry in &mut self.routes {
            if matches!(
                entry.route.mode,
                WebRouteMode::Server(ServerRenderPolicy { cache_scope: None })
            ) {
                entry.route.mode = mode.clone();
            }
        }
    }
}

fn render_widget_node<S, W>(
    widget: &W,
    ctx: &ServerRenderContext<'_>,
    mut state: S,
) -> Result<ServerRenderedNode>
where
    S: GlobalState + 'static,
    W: Clone + Into<Widget>,
{
    let runtime = RuntimeState::default();
    let mut env = Env::default();
    env.theme = ctx.theme.clone();
    env.viewport_size = ctx.viewport_size;
    let mut executed_jobs = BTreeSet::new();
    let mut pending_action = ctx.action.cloned();
    let mut final_node = None;
    let mut final_resources = Vec::new();

    for pass in 0..=ctx.render_pass_limit {
        let view = View::new(&state, &runtime, &env, None);
        let mut build_ctx = BuildCtx::<S>::new();
        let node = fission_core::build::enter(&mut build_ctx, &view, || (*widget).clone().into());

        if let Some(action) = pending_action.take() {
            build_ctx.registry.dispatch(
                &mut state,
                &action.action,
                WidgetId::from_u128(action.target_node),
            )?;
            continue;
        }

        let resources = build_ctx.resources.take();
        let dispatched = drain_server_jobs(
            &resources,
            &mut build_ctx,
            &mut state,
            ctx.jobs,
            &mut executed_jobs,
        )?;
        final_node = Some(node);
        final_resources = resources;
        if !dispatched {
            break;
        }
        if pass == ctx.render_pass_limit {
            anyhow::bail!(
                "server route `{}` exceeded render pass limit {} while draining jobs",
                ctx.route_path,
                ctx.render_pass_limit
            );
        }
    }

    Ok(ServerRenderedNode {
        node: final_node.unwrap_or_else(|| {
            let view = View::new(&state, &runtime, &env, None);
            let mut build_ctx = BuildCtx::<S>::new();
            fission_core::build::enter(&mut build_ctx, &view, || (*widget).clone().into())
        }),
        resources: final_resources,
    })
}

fn drain_server_jobs<S: GlobalState>(
    resources: &[RuntimeResourceDeclaration],
    build_ctx: &mut BuildCtx<S>,
    state: &mut S,
    jobs: &ServerJobRegistry,
    executed_jobs: &mut BTreeSet<String>,
) -> Result<bool> {
    let mut dispatched = false;
    for resource in resources {
        let RuntimeResourceKind::Job(job) = &resource.kind else {
            continue;
        };
        let Effect::Job(payload) = &job.effect.effect else {
            continue;
        };
        let execution_key = format!(
            "{}:{}:{}",
            resource.key,
            payload.job_name,
            resource
                .deps
                .as_ref()
                .map(|deps| blake3::hash(deps).to_hex().to_string())
                .unwrap_or_default()
        );
        if !executed_jobs.insert(execution_key) {
            continue;
        }
        jobs.require_job(&payload.job_name)?;
        let result = jobs.run(
            &payload.job_name,
            payload.payload.clone(),
            crate::ServerJobCtx {
                req_id: job.effect.req_id,
                resource_key: resource.key.clone(),
            },
        );
        match result {
            Ok(result_payload) => {
                if let Some(action) = &job.effect.on_ok {
                    build_ctx.registry.dispatch_with_input(
                        state,
                        action,
                        WidgetId::from_u128(0),
                        &ActionInput::JobOk {
                            job_name: payload.job_name.clone(),
                            req_id: job.effect.req_id,
                            payload: result_payload,
                        },
                    )?;
                    dispatched = true;
                }
            }
            Err(error) => {
                if let Some(action) = &job.effect.on_err {
                    build_ctx.registry.dispatch_with_input(
                        state,
                        action,
                        WidgetId::from_u128(0),
                        &ActionInput::JobErr {
                            job_name: payload.job_name.clone(),
                            req_id: job.effect.req_id,
                            payload: error.payload,
                            message: error.message,
                        },
                    )?;
                    dispatched = true;
                }
            }
        }
    }
    Ok(dispatched)
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
