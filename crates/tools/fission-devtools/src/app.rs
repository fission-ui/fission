use std::sync::Arc;

use anyhow::{Context, Result};
use fission_core::internal::{BuildCtx, InternalLoweringCx};
use fission_core::ui::{Container, Overlay, ZStack};
use fission_core::{
    ActionRegistry, Env, GlobalState, Runtime, View, Widget, WidgetId, WidgetIdExt,
};
use serde::{de::DeserializeOwned, Serialize};

use crate::protocol::{
    AppAction, AppFrame, DispatchResult, StateSnapshot, WorkerCommand, WorkerError,
    WorkerHandshake, WorkerOutput, APP_WORKER_PROTOCOL_VERSION,
};

type RuntimeConfigurator<S> =
    Arc<dyn Fn(&mut Runtime, &mut ActionRegistry<S>) -> Result<()> + Send + Sync>;
type EnvironmentSynchronizer<S> = Arc<dyn Fn(&S, &mut Env) + Send + Sync>;
type RouteReader<S> = Arc<dyn Fn(&S) -> Option<String> + Send + Sync>;

pub struct DevtoolsApp<S, W>
where
    S: GlobalState,
    W: Clone + Into<Widget>,
{
    application_id: String,
    application_name: String,
    state_schema: String,
    initial_state: Option<S>,
    root: W,
    env: Env,
    configure_runtime: RuntimeConfigurator<S>,
    sync_env: EnvironmentSynchronizer<S>,
    route: RouteReader<S>,
}

impl<S, W> DevtoolsApp<S, W>
where
    S: GlobalState,
    W: Clone + Into<Widget>,
{
    pub fn new(
        application_id: impl Into<String>,
        application_name: impl Into<String>,
        state_schema: impl Into<String>,
        initial_state: S,
        root: W,
    ) -> Self {
        Self {
            application_id: application_id.into(),
            application_name: application_name.into(),
            state_schema: state_schema.into(),
            initial_state: Some(initial_state),
            root,
            env: Env::default(),
            configure_runtime: Arc::new(|_, _| Ok(())),
            sync_env: Arc::new(|_, _| {}),
            route: Arc::new(|_| None),
        }
    }

    pub fn with_env(mut self, env: Env) -> Self {
        self.env = env;
        self
    }

    pub fn with_runtime<F>(mut self, configure: F) -> Self
    where
        F: Fn(&mut Runtime, &mut ActionRegistry<S>) -> Result<()> + Send + Sync + 'static,
    {
        self.configure_runtime = Arc::new(configure);
        self
    }

    pub fn with_sync_env<F>(mut self, sync: F) -> Self
    where
        F: Fn(&S, &mut Env) + Send + Sync + 'static,
    {
        self.sync_env = Arc::new(sync);
        self
    }

    pub fn with_route<F>(mut self, route: F) -> Self
    where
        F: Fn(&S) -> Option<String> + Send + Sync + 'static,
    {
        self.route = Arc::new(route);
        self
    }
}

pub(crate) struct AppWorker<S, W>
where
    S: GlobalState,
    W: Clone + Into<Widget>,
{
    config: DevtoolsApp<S, W>,
    runtime: Runtime,
    generation: u64,
    warned_custom_render_objects: bool,
}

impl<S, W> AppWorker<S, W>
where
    S: GlobalState + Serialize + DeserializeOwned + 'static,
    W: Clone + Into<Widget> + 'static,
{
    pub(crate) fn new(mut config: DevtoolsApp<S, W>, generation: u64) -> Result<Self> {
        let mut runtime = Runtime::default();
        runtime.add_app_state(Box::new(
            config
                .initial_state
                .take()
                .expect("Fission Developer app state may only be installed once"),
        ))?;
        let mut registry = ActionRegistry::new();
        (config.configure_runtime)(&mut runtime, &mut registry)?;
        runtime.absorb_persistent_registry(registry);
        Ok(Self {
            config,
            runtime,
            generation,
            warned_custom_render_objects: false,
        })
    }

    pub(crate) fn handle(&mut self, command: WorkerCommand) -> Result<WorkerOutput, WorkerError> {
        match command {
            WorkerCommand::Handshake => Ok(WorkerOutput::Handshake(WorkerHandshake {
                protocol_version: APP_WORKER_PROTOCOL_VERSION,
                generation: self.generation,
                application_id: self.config.application_id.clone(),
                state_schema: self.config.state_schema.clone(),
            })),
            WorkerCommand::Snapshot => self.snapshot().map(WorkerOutput::Snapshot),
            WorkerCommand::Restore { snapshot } => {
                self.restore(snapshot)?;
                Ok(WorkerOutput::Restored)
            }
            WorkerCommand::Build => self.build().map(WorkerOutput::Frame),
            WorkerCommand::Dispatch { dispatch } => {
                self.dispatch(dispatch).map(WorkerOutput::Dispatched)
            }
            WorkerCommand::Shutdown => Ok(WorkerOutput::ShuttingDown),
        }
    }

    fn snapshot(&self) -> Result<StateSnapshot, WorkerError> {
        let state = self.state()?;
        let payload = serde_json::to_vec(state).map_err(|error| {
            WorkerError::new("snapshot_failed", format!("state encoding failed: {error}"))
        })?;
        Ok(StateSnapshot {
            application_id: self.config.application_id.clone(),
            schema: self.config.state_schema.clone(),
            payload,
        })
    }

    fn restore(&mut self, snapshot: StateSnapshot) -> Result<(), WorkerError> {
        if snapshot.application_id != self.config.application_id
            || snapshot.schema != self.config.state_schema
        {
            return Err(WorkerError::new(
                "incompatible_snapshot",
                format!(
                    "expected {}/{}, received {}/{}",
                    self.config.application_id,
                    self.config.state_schema,
                    snapshot.application_id,
                    snapshot.schema,
                ),
            ));
        }
        let restored = serde_json::from_slice(&snapshot.payload).map_err(|error| {
            WorkerError::new("restore_failed", format!("state decoding failed: {error}"))
        })?;
        *self.state_mut()? = restored;
        Ok(())
    }

    fn dispatch(&mut self, dispatch: AppAction) -> Result<DispatchResult, WorkerError> {
        let input = dispatch.decode_input()?;
        self.runtime
            .dispatch_with_input(dispatch.action, dispatch.target, &input)
            .map_err(|error| WorkerError::new("dispatch_failed", error.to_string()))?;
        let discarded_effects = self.runtime.discard_pending_effects();
        if discarded_effects > 0 {
            eprintln!(
                "Fission Developer MVP discarded {discarded_effects} host effect(s); effect bridging is not enabled yet"
            );
        }
        Ok(DispatchResult {
            state_changed: true,
            frame: self.build()?,
        })
    }

    fn build(&mut self) -> Result<AppFrame, WorkerError> {
        {
            let sync_env = self.config.sync_env.clone();
            let state = self.runtime.get_app_state::<S>().ok_or_else(|| {
                WorkerError::new("missing_state", "application state is unavailable")
            })?;
            sync_env(state, &mut self.config.env);
        }
        let route = (self.config.route)(self.state()?);
        let tree = {
            let state = self.state()?;
            let view = View::new(state, &self.runtime.runtime_state, &self.config.env, None);
            let mut ctx = BuildCtx::new();
            let root = self.config.root.clone();
            let tree = fission_core::build::enter(&mut ctx, &view, || root.into());
            let portals = ctx
                .take_portals()
                .into_iter()
                .map(|(id, widget)| {
                    let widget = Container::new(widget)
                        .width(self.config.env.viewport_size.width)
                        .height(self.config.env.viewport_size.height);
                    match id {
                        Some(id) => widget
                            .id(WidgetId::derived(id.as_u128(), &[0x0000_F001]))
                            .into(),
                        None => widget.into(),
                    }
                })
                .collect::<Vec<Widget>>();
            self.runtime.clear_reducers();
            self.runtime.absorb_registry(ctx.registry);
            if portals.is_empty() {
                tree
            } else {
                let viewport = self.config.env.viewport_size;
                Overlay {
                    id: None,
                    content: Container::new(tree)
                        .width(viewport.width)
                        .height(viewport.height)
                        .into(),
                    overlay: ZStack {
                        id: None,
                        children: portals,
                    }
                    .into(),
                }
                .into()
            }
        };
        let runtime_state = &self.runtime.runtime_state;
        let mut cx = InternalLoweringCx::new(&self.config.env, runtime_state, None, None);
        let root = fission_core::internal::lower_widget(&tree, &mut cx);
        cx.ir.root = Some(root);
        let unsupported_render_objects = cx.ir.custom_render_objects.len();
        if unsupported_render_objects > 0 {
            if !self.warned_custom_render_objects {
                eprintln!(
                    "Fission Developer MVP omitted {unsupported_render_objects} generation-local custom render object(s); their serializable Core IR remains active"
                );
                self.warned_custom_render_objects = true;
            }
            cx.ir.custom_render_objects.clear();
        }
        Ok(AppFrame {
            application_name: self.config.application_name.clone(),
            route,
            generation: self.generation,
            ir: cx.ir,
        })
    }

    fn state(&self) -> Result<&S, WorkerError> {
        self.runtime
            .get_app_state::<S>()
            .ok_or_else(|| WorkerError::new("missing_state", "application state is unavailable"))
    }

    fn state_mut(&mut self) -> Result<&mut S, WorkerError> {
        self.runtime
            .get_app_state_mut::<S>()
            .ok_or_else(|| WorkerError::new("missing_state", "application state is unavailable"))
    }
}

pub fn run<S, W>(config: DevtoolsApp<S, W>) -> Result<()>
where
    S: GlobalState + Serialize + DeserializeOwned + 'static,
    W: Clone + Into<Widget> + 'static,
{
    let generation = std::env::var("FISSION_DEV_GENERATION")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or_default();
    let mut worker = AppWorker::new(config, generation)?;
    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout().lock();
    use std::io::{BufRead, Write};

    for line in stdin.lock().lines() {
        let line = line.context("failed to read Fission Developer request")?;
        let request = match crate::decode_line::<crate::WorkerRequest>(&line) {
            Ok(request) => request,
            Err(error) => {
                eprintln!("invalid Fission Developer request: {error}");
                continue;
            }
        };
        let should_stop = matches!(request.command, WorkerCommand::Shutdown);
        let response = crate::WorkerResponse {
            request_id: request.request_id,
            result: worker.handle(request.command),
        };
        stdout.write_all(crate::encode_line(&response)?.as_bytes())?;
        stdout.flush()?;
        if should_stop {
            break;
        }
    }
    Ok(())
}
