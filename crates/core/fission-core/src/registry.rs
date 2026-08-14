use crate::{
    action::video::{
        VideoPause, VideoPlay, VideoSeek, VideoSetMuted, VideoSetRate, VideoSetVolume, VideoStop,
    },
    async_runtime::{
        JobRef, JobRequestPayload, JobSpec, ServiceBindings, ServiceSlot, ServiceSpec,
        ServiceStartPayload,
    },
    context::{Effects, ReducerContext},
    effect::{ActionInput, Effect, EffectEnvelope},
    ui::{VideoAudioOptions, Widget},
    Action, ActionEnvelope, ActionId, BoxedReducer, GlobalState,
};
use anyhow::{anyhow, Result};
use fission_diagnostics::prelude as diag;
use fission_ir::WidgetId;
use serde::Serialize;
use std::any::TypeId;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::error::Error;
use std::fmt;

#[derive(Debug)]
pub(crate) struct ActionDeserializationError;

impl ActionDeserializationError {
    pub(crate) fn new(_source: serde_json::Error) -> Self {
        Self
    }
}

impl fmt::Display for ActionDeserializationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("failed to deserialize action payload")
    }
}

impl Error for ActionDeserializationError {}

pub(crate) fn emit_action_dispatch_failure(
    action_id: ActionId,
    target: WidgetId,
    error: &anyhow::Error,
) {
    let failure_kind = if error.downcast_ref::<ActionDeserializationError>().is_some() {
        "action_deserialization"
    } else {
        "reducer_dispatch"
    };
    #[cfg(not(target_arch = "wasm32"))]
    eprintln!(
        "{}",
        action_dispatch_failure_message(action_id, target, failure_kind)
    );
    diag::emit(
        diag::DiagCategory::Input,
        diag::DiagLevel::Error,
        diag::DiagEventKind::ActionDispatchFailed {
            action_id: action_id.as_u128(),
            target: target.as_u128(),
            failure_kind: failure_kind.into(),
        },
    );
}

#[cfg(any(not(target_arch = "wasm32"), test))]
fn action_dispatch_failure_message(
    action_id: ActionId,
    target: WidgetId,
    failure_kind: &'static str,
) -> String {
    format!(
        "Fission action dispatch failed: action_id={} target={} failure_kind={failure_kind}",
        action_id.as_u128(),
        target.as_u128()
    )
}

/// The canonical 3-argument handler signature for modern reducers.
///
/// ```rust,ignore
/// fn handle_increment(state: &mut Counter, _: Increment, _ctx: &mut ReducerContext<Counter>) {
///     state.count += 1;
/// }
/// ```
pub type Handler<S, A> = for<'a, 'b, 'c> fn(&mut S, A, &mut ReducerContext<'a, 'b, 'c, S>);

/// Trait that allows both 2-argument (legacy) and 3-argument (modern) handler
/// functions to be used with [`ActionRegistry::register`] and
/// [`BuildCtxHandle::bind`](crate::build::BuildCtxHandle::bind).
pub trait IntoHandler<S: GlobalState, A> {
    /// Invoke the handler with the given state, action, and context.
    fn call<'a, 'b, 'c>(&self, state: &mut S, action: A, ctx: &mut ReducerContext<'a, 'b, 'c, S>);
}

// Impl for Legacy (2-arg)
impl<S: GlobalState, A> IntoHandler<S, A> for fn(&mut S, A) {
    fn call<'a, 'b, 'c>(&self, state: &mut S, action: A, _ctx: &mut ReducerContext<'a, 'b, 'c, S>) {
        (self)(state, action);
    }
}

// Impl for Modern (3-arg)
impl<S: GlobalState, A> IntoHandler<S, A>
    for for<'a, 'b, 'c> fn(&mut S, A, &mut ReducerContext<'a, 'b, 'c, S>)
{
    fn call<'a, 'b, 'c>(&self, state: &mut S, action: A, ctx: &mut ReducerContext<'a, 'b, 'c, S>) {
        (self)(state, action, ctx);
    }
}

// Internal typed reducer storage
type TypedReducer<S> = Box<
    dyn for<'a, 'b, 'c> Fn(
            &mut S,
            &ActionEnvelope,
            WidgetId,
            &mut Effects<'a, S>,
            &'b ActionInput,
        ) -> Result<()>
        + Send
        + Sync,
>;

fn into_boxed_runtime_reducer<S: GlobalState>(
    state_type_id: TypeId,
    typed_reducer: TypedReducer<S>,
) -> BoxedReducer {
    Box::new(
        move |app_states: &mut HashMap<TypeId, Box<dyn GlobalState>>,
              action: &ActionEnvelope,
              target: WidgetId,
              out_effects: &mut Vec<EffectEnvelope>,
              input: &ActionInput,
              callback_registry|
              -> Result<()> {
            if let Some(state_box) = app_states.get_mut(&state_type_id) {
                let concrete_state = state_box
                    .downcast_mut::<S>()
                    .ok_or_else(|| anyhow!("Failed to downcast GlobalState to concrete type"))?;

                let mut effects_builder = Effects::new_runtime(0, callback_registry.clone());

                typed_reducer(concrete_state, action, target, &mut effects_builder, input)?;

                out_effects.extend(effects_builder.out);

                Ok(())
            } else {
                anyhow::bail!("Target GlobalState for reducer not found in runtime.");
            }
        },
    )
}

/// A per-frame collection of action handlers registered during widget building.
///
/// `ActionRegistry` is populated by [`BuildCtxHandle::bind`](crate::build::BuildCtxHandle::bind)
/// calls. After the widget
/// tree is built, the registry is absorbed into the [`Runtime`](crate::Runtime)
/// via [`Runtime::absorb_registry`](crate::Runtime::absorb_registry).
pub struct ActionRegistry<S: GlobalState> {
    handlers: BTreeMap<ActionId, Vec<TypedReducer<S>>>,
    bound_handlers: BTreeMap<ActionId, TypedReducer<S>>,
    runtime_handlers: BTreeMap<ActionId, BoxedReducer>,
}

impl<S: GlobalState> Default for ActionRegistry<S> {
    fn default() -> Self {
        Self {
            handlers: BTreeMap::new(),
            bound_handlers: BTreeMap::new(),
            runtime_handlers: BTreeMap::new(),
        }
    }
}

impl<S: GlobalState> ActionRegistry<S> {
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers an explicit handler for an action type.
    ///
    /// Every registered handler runs when the action is dispatched. Declarative
    /// widget bindings use a separate exactly-once path; call `register` only
    /// when multicast handling is intentional.
    pub fn register<A: Action, H: IntoHandler<S, A> + Send + Sync + 'static>(
        &mut self,
        handler: H,
    ) {
        self.register_with_id(A::static_id(), handler);
    }

    pub(crate) fn register_with_id<A: Action, H: IntoHandler<S, A> + Send + Sync + 'static>(
        &mut self,
        action_id: ActionId,
        handler: H,
    ) {
        self.handlers
            .entry(action_id)
            .or_default()
            .push(Self::typed_reducer(handler));
    }

    pub(crate) fn register_bound<A: Action, H: IntoHandler<S, A> + Send + Sync + 'static>(
        &mut self,
        handler: H,
    ) {
        self.bound_handlers
            .entry(A::static_id())
            .or_insert_with(|| Self::typed_reducer(handler));
    }

    fn typed_reducer<A: Action, H: IntoHandler<S, A> + Send + Sync + 'static>(
        handler: H,
    ) -> TypedReducer<S> {
        Box::new(
            move |state: &mut S,
                  envelope: &ActionEnvelope,
                  _target,
                  effects,
                  input|
                  -> Result<()> {
                let action: A = serde_json::from_slice(&envelope.payload)
                    .map_err(ActionDeserializationError::new)?;

                let mut ctx = ReducerContext { effects, input };

                handler.call(state, action, &mut ctx);
                Ok(())
            },
        )
    }

    pub fn action_ids(&self) -> Vec<ActionId> {
        self.handlers
            .keys()
            .chain(self.bound_handlers.keys())
            .chain(self.runtime_handlers.keys())
            .copied()
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect()
    }

    pub(crate) fn register_runtime_reducer(&mut self, action_id: ActionId, reducer: BoxedReducer) {
        self.runtime_handlers.entry(action_id).or_insert(reducer);
    }

    pub fn dispatch_with_input(
        &mut self,
        state: &mut S,
        action: &ActionEnvelope,
        target: WidgetId,
        input: &ActionInput,
    ) -> Result<Vec<EffectEnvelope>> {
        let mut effects_builder = Effects::new_headless(0);
        let target: WidgetId = target.into();
        let dispatch_result = (|| {
            if let Some(reducers) = self.handlers.get_mut(&action.id) {
                reducers.iter_mut().try_for_each(|reducer| {
                    reducer(state, action, target, &mut effects_builder, input)
                })?;
            }
            if let Some(reducer) = self.bound_handlers.get_mut(&action.id) {
                reducer(state, action, target, &mut effects_builder, input)?;
            }
            Ok(())
        })();
        if let Err(error) = dispatch_result {
            emit_action_dispatch_failure(action.id, target, &error);
            return Err(error);
        }
        Ok(effects_builder.out)
    }

    pub fn dispatch(
        &mut self,
        state: &mut S,
        action: &ActionEnvelope,
        target: WidgetId,
    ) -> Result<Vec<EffectEnvelope>> {
        self.dispatch_with_input(state, action, target, &ActionInput::None)
    }

    pub(crate) fn into_runtime_reducers(self) -> HashMap<ActionId, Vec<BoxedReducer>> {
        let mut runtime_reducers: HashMap<ActionId, Vec<BoxedReducer>> = HashMap::new();
        let state_type_id = TypeId::of::<S>();

        for (action_id, reducer) in self.runtime_handlers {
            runtime_reducers.entry(action_id).or_default().push(reducer);
        }

        for (action_id, typed_reducers) in self.handlers {
            for typed_reducer in typed_reducers {
                runtime_reducers
                    .entry(action_id)
                    .or_default()
                    .push(into_boxed_runtime_reducer(state_type_id, typed_reducer));
            }
        }

        for (action_id, typed_reducer) in self.bound_handlers {
            runtime_reducers
                .entry(action_id)
                .or_default()
                .push(into_boxed_runtime_reducer(state_type_id, typed_reducer));
        }
        runtime_reducers
    }
}

/// Registration data for a [`Video`](crate::ui::Video) widget collected during
/// widget building.
#[derive(Clone, Debug)]
pub struct VideoRegistration {
    /// The stable widget identity of the video node.
    pub node_id: WidgetId,
    /// URL or asset path to the video file.
    pub source: String,
    /// Whether to start playing automatically.
    pub autoplay: bool,
    /// Whether to loop playback.
    pub loop_playback: bool,
    /// Audio-session behavior requested by this video.
    pub audio: VideoAudioOptions,
}

/// Registration data for a platform web view collected during widget building.
#[derive(Clone, Debug)]
pub struct WebRegistration {
    /// The stable widget identity of the web view node.
    pub node_id: WidgetId,
    /// The URL to load.
    pub url: String,
    /// Optional custom user-agent string.
    pub user_agent: Option<String>,
}

/// Z-order layer for portal entries.
///
/// Portals are sorted by layer (then by registration order within a layer).
/// Higher layers paint on top of lower layers.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum PortalLayer {
    /// Default overlay layer.
    Default = 0,
    /// Modal dialog layer.
    Modal = 100,
    /// Flyout / dropdown layer.
    Flyout = 200,
    /// Toast notification layer (topmost).
    Toast = 300,
}

/// An entry in the portal overlay stack.
///
/// Created by [`crate::internal::BuildCtx::register_portal`] and friends.
#[derive(Clone, Debug)]
pub struct PortalEntry {
    /// Which overlay layer this portal belongs to.
    pub layer: PortalLayer,
    /// Insertion order (for stable ordering within a layer).
    pub seq: u64,
    /// Optional stable identity.
    pub id: Option<WidgetId>,
    /// The portal's widget tree.
    pub node: Widget,
}

/// The mutable context available during `impl From<Component> for Widget`.
///
#[derive(Clone, Copy)]
pub struct VideoControlCtx {
    pub(crate) target: WidgetId,
}

impl VideoControlCtx {
    pub fn play(&self) -> ActionEnvelope {
        let action = VideoPlay {
            target: self.target,
        };
        ActionEnvelope {
            id: VideoPlay::static_id(),
            payload: action.encode(),
        }
    }

    pub fn pause(&self) -> ActionEnvelope {
        let action = VideoPause {
            target: self.target,
        };
        ActionEnvelope {
            id: VideoPause::static_id(),
            payload: action.encode(),
        }
    }

    pub fn stop(&self) -> ActionEnvelope {
        let action = VideoStop {
            target: self.target,
        };
        ActionEnvelope {
            id: VideoStop::static_id(),
            payload: action.encode(),
        }
    }

    pub fn seek_to(&self, position_ms: u64) -> ActionEnvelope {
        let action = VideoSeek {
            target: self.target,
            position_ms,
        };
        ActionEnvelope {
            id: VideoSeek::static_id(),
            payload: action.encode(),
        }
    }

    pub fn set_rate(&self, rate: f32) -> ActionEnvelope {
        let action = VideoSetRate {
            target: self.target,
            rate,
        };
        ActionEnvelope {
            id: VideoSetRate::static_id(),
            payload: action.encode(),
        }
    }

    pub fn set_volume(&self, volume: f32) -> ActionEnvelope {
        let action = VideoSetVolume {
            target: self.target,
            volume,
        };
        ActionEnvelope {
            id: VideoSetVolume::static_id(),
            payload: action.encode(),
        }
    }

    pub fn set_muted(&self, muted: bool) -> ActionEnvelope {
        let action = VideoSetMuted {
            target: self.target,
            muted,
        };
        ActionEnvelope {
            id: VideoSetMuted::static_id(),
            payload: action.encode(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ResourceKey(String);

impl ResourceKey {
    pub fn new(name: impl Into<String>) -> Self {
        Self(name.into())
    }

    pub fn widget(name: impl AsRef<str>, id: WidgetId) -> Self {
        Self(format!("widget:{}:{}", id.as_u128(), name.as_ref()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResourcePolicy {
    PreserveOnChange,
    RestartOnChange,
}

impl Default for ResourcePolicy {
    fn default() -> Self {
        Self::RestartOnChange
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct RuntimeResourceDeclaration {
    pub key: String,
    pub deps: Option<Vec<u8>>,
    pub policy: ResourcePolicy,
    pub kind: RuntimeResourceKind,
}

#[derive(Clone, Debug, PartialEq)]
pub enum RuntimeResourceKind {
    Job(JobResource),
    Service(ServiceResource),
    Timer(TimerResource),
}

#[derive(Clone, Debug, PartialEq)]
pub struct JobResource {
    pub key: ResourceKey,
    pub effect: EffectEnvelope,
    pub deps: Option<Vec<u8>>,
    pub policy: ResourcePolicy,
}

impl JobResource {
    pub fn new<J: JobSpec>(key: ResourceKey, job: JobRef<J>, request: J::Request) -> Self {
        let payload =
            serde_json::to_vec(&request).expect("job resource request serialization must succeed");
        Self {
            key,
            effect: EffectEnvelope {
                req_id: 0,
                effect: Effect::Job(JobRequestPayload {
                    job_name: job.name.to_string(),
                    payload,
                }),
                on_ok: None,
                on_err: None,
                service_bindings: None,
                resource: None,
            },
            deps: None,
            policy: ResourcePolicy::RestartOnChange,
        }
    }

    pub fn deps<T: Serialize>(mut self, deps: T) -> Self {
        self.deps =
            Some(serde_json::to_vec(&deps).expect("resource deps serialization must succeed"));
        self
    }

    pub fn preserve_on_change(mut self) -> Self {
        self.policy = ResourcePolicy::PreserveOnChange;
        self
    }

    pub fn restart_on_change(mut self) -> Self {
        self.policy = ResourcePolicy::RestartOnChange;
        self
    }

    pub fn on_ok(mut self, action: ActionEnvelope) -> Self {
        self.effect.on_ok = Some(action);
        self
    }

    pub fn on_err(mut self, action: ActionEnvelope) -> Self {
        self.effect.on_err = Some(action);
        self
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ServiceResource {
    pub key: ResourceKey,
    pub effect: EffectEnvelope,
    pub deps: Option<Vec<u8>>,
    pub policy: ResourcePolicy,
}

impl ServiceResource {
    pub fn new<Svc: ServiceSpec>(
        key: ResourceKey,
        slot: ServiceSlot<Svc>,
        config: Svc::Config,
    ) -> Self {
        let config = serde_json::to_vec(&config)
            .expect("service resource config serialization must succeed");
        Self {
            key,
            effect: EffectEnvelope {
                req_id: 0,
                effect: Effect::StartService(ServiceStartPayload {
                    service_name: slot.ty.name.to_string(),
                    slot_key: slot.slot_key().to_string(),
                    config,
                }),
                on_ok: None,
                on_err: None,
                service_bindings: Some(ServiceBindings::default()),
                resource: None,
            },
            deps: None,
            policy: ResourcePolicy::RestartOnChange,
        }
    }

    pub fn deps<T: Serialize>(mut self, deps: T) -> Self {
        self.deps =
            Some(serde_json::to_vec(&deps).expect("resource deps serialization must succeed"));
        self
    }

    pub fn preserve_on_change(mut self) -> Self {
        self.policy = ResourcePolicy::PreserveOnChange;
        self
    }

    pub fn restart_on_change(mut self) -> Self {
        self.policy = ResourcePolicy::RestartOnChange;
        self
    }

    pub fn on_started(mut self, action: ActionEnvelope) -> Self {
        if let Some(bindings) = self.effect.service_bindings.as_mut() {
            bindings.on_started = Some(action);
        }
        self
    }

    pub fn on_start_failed(mut self, action: ActionEnvelope) -> Self {
        if let Some(bindings) = self.effect.service_bindings.as_mut() {
            bindings.on_start_failed = Some(action);
        }
        self
    }

    pub fn on_event(mut self, action: ActionEnvelope) -> Self {
        if let Some(bindings) = self.effect.service_bindings.as_mut() {
            bindings.on_event = Some(action);
        }
        self
    }

    pub fn on_stopped(mut self, action: ActionEnvelope) -> Self {
        if let Some(bindings) = self.effect.service_bindings.as_mut() {
            bindings.on_stopped = Some(action);
        }
        self
    }

    pub fn on_command_ok(mut self, action: ActionEnvelope) -> Self {
        if let Some(bindings) = self.effect.service_bindings.as_mut() {
            bindings.on_command_ok = Some(action);
        }
        self
    }

    pub fn on_command_err(mut self, action: ActionEnvelope) -> Self {
        if let Some(bindings) = self.effect.service_bindings.as_mut() {
            bindings.on_command_err = Some(action);
        }
        self
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TimerResource {
    pub key: ResourceKey,
    pub interval_ms: u64,
    pub payload: Vec<u8>,
    pub on_tick: Option<ActionEnvelope>,
    pub deps: Option<Vec<u8>>,
    pub immediate: bool,
    pub policy: ResourcePolicy,
}

impl TimerResource {
    pub fn new<T: Serialize>(key: ResourceKey, interval: std::time::Duration, payload: T) -> Self {
        Self {
            key,
            interval_ms: interval.as_millis() as u64,
            payload: serde_json::to_vec(&payload)
                .expect("timer resource payload serialization must succeed"),
            on_tick: None,
            deps: None,
            immediate: false,
            policy: ResourcePolicy::RestartOnChange,
        }
    }

    pub fn deps<T: Serialize>(mut self, deps: T) -> Self {
        self.deps =
            Some(serde_json::to_vec(&deps).expect("resource deps serialization must succeed"));
        self
    }

    pub fn preserve_on_change(mut self) -> Self {
        self.policy = ResourcePolicy::PreserveOnChange;
        self
    }

    pub fn restart_on_change(mut self) -> Self {
        self.policy = ResourcePolicy::RestartOnChange;
        self
    }

    pub fn immediate(mut self) -> Self {
        self.immediate = true;
        self
    }

    pub fn on_tick(mut self, action: ActionEnvelope) -> Self {
        self.on_tick = Some(action);
        self
    }
}

#[derive(Default)]
pub struct ResourceRegistry {
    declarations: Vec<RuntimeResourceDeclaration>,
    seen_keys: HashMap<String, usize>,
}

impl ResourceRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn job(&mut self, resource: JobResource) {
        self.push(RuntimeResourceDeclaration {
            key: resource.key.as_str().to_string(),
            deps: resource.deps.clone(),
            policy: resource.policy,
            kind: RuntimeResourceKind::Job(resource),
        });
    }

    pub fn service(&mut self, resource: ServiceResource) {
        self.push(RuntimeResourceDeclaration {
            key: resource.key.as_str().to_string(),
            deps: resource.deps.clone(),
            policy: resource.policy,
            kind: RuntimeResourceKind::Service(resource),
        });
    }

    pub fn timer(&mut self, resource: TimerResource) {
        self.push(RuntimeResourceDeclaration {
            key: resource.key.as_str().to_string(),
            deps: resource.deps.clone(),
            policy: resource.policy,
            kind: RuntimeResourceKind::Timer(resource),
        });
    }

    pub fn take(&mut self) -> Vec<RuntimeResourceDeclaration> {
        self.seen_keys.clear();
        std::mem::take(&mut self.declarations)
    }

    fn push(&mut self, declaration: RuntimeResourceDeclaration) {
        if let Some(index) = self.seen_keys.get(&declaration.key) {
            panic!(
                "duplicate runtime resource declaration for key '{}' at index {}",
                declaration.key, index
            );
        }
        let index = self.declarations.len();
        self.seen_keys.insert(declaration.key.clone(), index);
        self.declarations.push(declaration);
    }
}

#[cfg(test)]
mod dispatch_failure_tests {
    use super::action_dispatch_failure_message;
    use crate::{ActionId, WidgetId};

    #[test]
    fn fallback_message_contains_only_sanitized_dispatch_metadata() {
        let message = action_dispatch_failure_message(
            ActionId::from_u128(7),
            WidgetId::from_u128(11),
            "action_deserialization",
        );

        assert_eq!(
            message,
            "Fission action dispatch failed: action_id=7 target=11 failure_kind=action_deserialization"
        );
    }
}
