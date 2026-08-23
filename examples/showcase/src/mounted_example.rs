use fission::prelude::*;
use fission_core::internal::BuildCtx;
use fission_core::scoped_action_handlers::{
    clear_scoped_action_handlers, register_scoped_action_handler,
};
use fission_core::{
    build, ActionEnvelope, ActionScopeId, ReducerContext, ResourceKey, Runtime,
    RuntimeResourceKind, ScopedActionResolution, View,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

type MountedRuntimes = HashMap<String, RuntimeSlot>;

static MOUNTED_RUNTIMES: OnceLock<Mutex<MountedRuntimes>> = OnceLock::new();

struct RuntimeSlot {
    generation: u64,
    runtime: Arc<Mutex<Runtime>>,
}

pub(crate) struct MountedExample<S, W>
where
    S: GlobalState + Default,
{
    pub(crate) key: &'static str,
    pub(crate) generation: u64,
    pub(crate) root: W,
    pub(crate) initial_state: fn() -> S,
    pub(crate) configure_env: Option<fn(&S, &mut Env)>,
}

impl<S, W> MountedExample<S, W>
where
    S: GlobalState + Default,
{
    pub(crate) fn new(key: &'static str, generation: u64, root: W) -> Self {
        Self {
            key,
            generation,
            root,
            initial_state: S::default,
            configure_env: None,
        }
    }

    pub(crate) fn with_state(mut self, initial_state: fn() -> S) -> Self {
        self.initial_state = initial_state;
        self
    }

    pub(crate) fn with_env(mut self, configure_env: fn(&S, &mut Env)) -> Self {
        self.configure_env = Some(configure_env);
        self
    }
}

impl<S, W> From<MountedExample<S, W>> for Widget
where
    S: GlobalState + Default,
    W: Into<Widget>,
{
    fn from(component: MountedExample<S, W>) -> Self {
        let (outer_ctx, outer_view) = fission::build::current::<crate::state::ShowcaseState>();
        let scope = ActionScopeId::from_name(component.key);
        let runtime_key = runtime_key::<S>(component.key);
        let runtime =
            mounted_runtime::<S>(component.key, component.generation, component.initial_state);
        let mut child_env = outer_view.env().clone();
        configure_child_viewport(&outer_view, &mut child_env);
        outer_ctx.register(
            on_mounted_resource_callback
                as fn(
                    &mut crate::state::ShowcaseState,
                    MountedResourceCallback,
                    &mut ReducerContext<crate::state::ShowcaseState>,
                ),
        );

        let (child, action_ids, mut child_ctx) = {
            let mut runtime_guard = runtime
                .lock()
                .expect("mounted example runtime mutex poisoned");
            let mut child_runtime_state = outer_view.runtime().clone();
            child_runtime_state.local_widget_state =
                runtime_guard.runtime_state.local_widget_state.clone();
            let child_state = runtime_guard
                .get_global_state::<S>()
                .expect("mounted example state must be registered");
            if let Some(configure_env) = component.configure_env {
                configure_env(child_state, &mut child_env);
            }

            let child_view = View::new(
                child_state,
                &child_runtime_state,
                &child_env,
                outer_view.layout(),
            );
            let mut child_ctx = BuildCtx::<S>::new();
            let root_id = WidgetId::explicit(&format!(
                "showcase.mount.{}.{}",
                component.key, component.generation
            ));
            let child = build::enter(&mut child_ctx, &child_view, || {
                let root: Widget = component.root.into();
                root.id(root_id)
            });
            let action_ids = child_ctx.registry.action_ids();

            runtime_guard.clear_reducers();
            runtime_guard.absorb_registry(std::mem::take(&mut child_ctx.registry));
            (child, action_ids, child_ctx)
        };

        forward_build_registrations(
            &outer_ctx,
            &mut child_ctx,
            scope,
            &runtime_key,
            component.generation,
        );
        install_action_handlers(scope, action_ids, runtime);

        ActionScope::new(scope, child).into()
    }
}

fn runtime_key<S: GlobalState>(key: &str) -> String {
    format!("{key}:{}", std::any::type_name::<S>())
}

fn mounted_runtime<S>(key: &str, generation: u64, initial_state: fn() -> S) -> Arc<Mutex<Runtime>>
where
    S: GlobalState + Default,
{
    let map = MOUNTED_RUNTIMES.get_or_init(|| Mutex::new(HashMap::new()));
    let mut map = map.lock().expect("mounted example registry mutex poisoned");
    let map_key = runtime_key::<S>(key);
    let slot = map.entry(map_key).or_insert_with(|| RuntimeSlot {
        generation,
        runtime: Arc::new(Mutex::new(
            Runtime::default().with_global_state(initial_state()),
        )),
    });
    if slot.generation != generation {
        *slot = RuntimeSlot {
            generation,
            runtime: Arc::new(Mutex::new(
                Runtime::default().with_global_state(initial_state()),
            )),
        };
    }
    slot.runtime.clone()
}

fn configure_child_viewport(
    outer_view: &ViewHandle<crate::state::ShowcaseState>,
    child_env: &mut Env,
) {
    let preview_id = match outer_view.state().preview_viewport {
        crate::state::PreviewViewport::Desktop => WidgetId::explicit("showcase.preview.desktop"),
        crate::state::PreviewViewport::Mobile => WidgetId::explicit("showcase.preview.mobile"),
    };
    if let Some(rect) = outer_view.get_rect(preview_id) {
        child_env.viewport_size = LayoutSize::new(rect.width(), rect.height());
        return;
    }

    let outer = outer_view.viewport_size();
    child_env.viewport_size = match outer_view.state().preview_viewport {
        crate::state::PreviewViewport::Desktop => LayoutSize::new(
            (outer.width * 0.7).max(480.0),
            (outer.height - 160.0).max(320.0),
        ),
        crate::state::PreviewViewport::Mobile => {
            LayoutSize::new(390.0, (outer.height - 160.0).max(320.0))
        }
    };
}

fn install_action_handlers(
    scope: ActionScopeId,
    action_ids: Vec<ActionId>,
    runtime: Arc<Mutex<Runtime>>,
) {
    clear_scoped_action_handlers(scope).expect("mounted example action scope must clear");
    for action_id in action_ids {
        let runtime = runtime.clone();
        register_scoped_action_handler(
            scope,
            action_id,
            Box::new(move |action, target, input| {
                let mut runtime = runtime
                    .lock()
                    .expect("mounted example runtime mutex poisoned");
                runtime.dispatch_with_input(action.clone(), target, input.unscoped())?;
                runtime.pending_effects.clear();
                Ok(ScopedActionResolution::Handled)
            }),
        )
        .expect("mounted example action handler must register");
    }
}

fn forward_build_registrations(
    outer_ctx: &BuildCtxHandle<crate::state::ShowcaseState>,
    child_ctx: &mut BuildCtx<impl GlobalState>,
    scope: ActionScopeId,
    runtime_key: &str,
    generation: u64,
) {
    for declaration in child_ctx.take_motion_declarations() {
        outer_ctx.register_motion(declaration);
    }
    for registration in child_ctx.take_video_registrations() {
        outer_ctx.register_video(registration);
    }
    for registration in child_ctx.take_web_registrations() {
        outer_ctx.register_web_view(registration);
    }
    for portal in child_ctx.portals.drain(..) {
        outer_ctx.register_portal_with_layer(
            portal.layer,
            portal.id,
            ActionScope::new(scope, portal.node).into(),
        );
    }
    let resources = child_ctx.take_resources();
    outer_ctx.with_resources(|outer_resources| {
        for resource in resources {
            match resource.kind {
                RuntimeResourceKind::Job(mut job) => {
                    prefix_resource_key(&mut job.key, runtime_key, generation);
                    job.effect.on_ok = job
                        .effect
                        .on_ok
                        .take()
                        .map(|callback| mounted_callback(runtime_key, callback));
                    job.effect.on_err = job
                        .effect
                        .on_err
                        .take()
                        .map(|callback| mounted_callback(runtime_key, callback));
                    outer_resources.job(job);
                }
                RuntimeResourceKind::Service(mut service) => {
                    prefix_resource_key(&mut service.key, runtime_key, generation);
                    if let Some(bindings) = service.effect.service_bindings.as_mut() {
                        bindings.on_started = bindings
                            .on_started
                            .take()
                            .map(|callback| mounted_callback(runtime_key, callback));
                        bindings.on_start_failed = bindings
                            .on_start_failed
                            .take()
                            .map(|callback| mounted_callback(runtime_key, callback));
                        bindings.on_event = bindings
                            .on_event
                            .take()
                            .map(|callback| mounted_callback(runtime_key, callback));
                        bindings.on_stopped = bindings
                            .on_stopped
                            .take()
                            .map(|callback| mounted_callback(runtime_key, callback));
                        bindings.on_command_ok = bindings
                            .on_command_ok
                            .take()
                            .map(|callback| mounted_callback(runtime_key, callback));
                        bindings.on_command_err = bindings
                            .on_command_err
                            .take()
                            .map(|callback| mounted_callback(runtime_key, callback));
                    }
                    outer_resources.service(service);
                }
                RuntimeResourceKind::Timer(mut timer) => {
                    prefix_resource_key(&mut timer.key, runtime_key, generation);
                    timer.on_tick = timer
                        .on_tick
                        .take()
                        .map(|callback| mounted_callback(runtime_key, callback));
                    outer_resources.timer(timer);
                }
            }
        }
    });
}

fn prefix_resource_key(key: &mut ResourceKey, runtime_key: &str, generation: u64) {
    *key = ResourceKey::new(format!(
        "showcase:{runtime_key}:{generation}:{}",
        key.as_str()
    ));
}

fn mounted_callback(runtime_key: &str, callback: ActionEnvelope) -> ActionEnvelope {
    MountedResourceCallback {
        runtime_key: runtime_key.to_string(),
        callback,
    }
    .into()
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct MountedResourceCallback {
    runtime_key: String,
    callback: ActionEnvelope,
}

impl Action for MountedResourceCallback {
    fn static_id() -> ActionId {
        ActionId::from_name("example_showcase::MountedResourceCallback")
    }
}

fn on_mounted_resource_callback(
    _state: &mut crate::state::ShowcaseState,
    action: MountedResourceCallback,
    ctx: &mut ReducerContext<crate::state::ShowcaseState>,
) {
    let Some(runtime) = MOUNTED_RUNTIMES
        .get()
        .and_then(|runtimes| runtimes.lock().ok())
        .and_then(|runtimes| {
            runtimes
                .get(&action.runtime_key)
                .map(|slot| slot.runtime.clone())
        })
    else {
        return;
    };
    let Ok(mut runtime) = runtime.lock() else {
        return;
    };
    let _ = runtime.dispatch_with_input(
        action.callback,
        WidgetId::from_u128(0),
        ctx.input.unscoped(),
    );
    runtime.pending_effects.clear();
}
