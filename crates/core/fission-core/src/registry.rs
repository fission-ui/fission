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
    ui::Node,
    Action, ActionEnvelope, ActionId, ActionScopeId, AppState, BoxedReducer,
};
use anyhow::{anyhow, Result};
use fission_ir::{NodeId, WidgetNodeId};
use serde::Serialize;
use std::any::TypeId;
use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

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
/// [`BuildCtx::bind`].
pub trait IntoHandler<S: AppState, A> {
    /// Invoke the handler with the given state, action, and context.
    fn call<'a, 'b, 'c>(&self, state: &mut S, action: A, ctx: &mut ReducerContext<'a, 'b, 'c, S>);
}

// Impl for Legacy (2-arg)
impl<S: AppState, A> IntoHandler<S, A> for fn(&mut S, A) {
    fn call<'a, 'b, 'c>(&self, state: &mut S, action: A, _ctx: &mut ReducerContext<'a, 'b, 'c, S>) {
        (self)(state, action);
    }
}

// Impl for Modern (3-arg)
impl<S: AppState, A> IntoHandler<S, A>
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
            NodeId,
            &mut Effects<'a, S>,
            &'b ActionInput,
        ) -> Result<()>
        + Send
        + Sync,
>;

/// Raw action handler used for mounted or external subtrees.
///
/// Unlike typed reducers, this receives the original action envelope and
/// dispatch target without deserializing the payload.
pub type RawActionHandler<S> = Box<
    dyn for<'a, 'b> Fn(
            &mut S,
            &ActionEnvelope,
            NodeId,
            &mut Effects<'a, S>,
            &'b ActionInput,
        ) -> Result<()>
        + Send
        + Sync,
>;

/// A per-frame collection of action handlers registered during widget building.
///
/// `ActionRegistry` is populated by [`BuildCtx::bind`] calls. After the widget
/// tree is built, the registry is absorbed into the [`Runtime`](crate::Runtime)
/// via [`Runtime::absorb_registry`](crate::Runtime::absorb_registry).
pub struct ActionRegistry<S: AppState> {
    handlers: BTreeMap<ActionId, Vec<TypedReducer<S>>>,
}

impl<S: AppState> Default for ActionRegistry<S> {
    fn default() -> Self {
        Self {
            handlers: BTreeMap::new(),
        }
    }
}

impl<S: AppState> ActionRegistry<S> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register<A: Action, H: IntoHandler<S, A> + Send + Sync + 'static>(
        &mut self,
        handler: H,
    ) {
        let action_id = A::static_id();

        let typed_reducer: TypedReducer<S> = Box::new(
            move |state: &mut S,
                  envelope: &ActionEnvelope,
                  _target,
                  effects,
                  input|
                  -> Result<()> {
                let action: A = serde_json::from_slice(&envelope.payload)
                    .map_err(|e| anyhow!("Failed to deserialize action: {}", e))?;

                let mut ctx = ReducerContext { effects, input };

                handler.call(state, action, &mut ctx);
                Ok(())
            },
        );

        self.handlers
            .entry(action_id)
            .or_default()
            .push(typed_reducer);
    }

    pub fn register_raw_action<F>(&mut self, action_id: ActionId, handler: F)
    where
        F: for<'a, 'b> Fn(
                &mut S,
                &ActionEnvelope,
                NodeId,
                &mut Effects<'a, S>,
                &'b ActionInput,
            ) -> Result<()>
            + Send
            + Sync
            + 'static,
    {
        self.handlers
            .entry(action_id)
            .or_default()
            .push(Box::new(handler));
    }

    pub fn register_scoped_raw_action<F>(
        &mut self,
        scope_id: ActionScopeId,
        action_id: ActionId,
        handler: F,
    ) where
        F: for<'a, 'b> Fn(
                &mut S,
                &ActionEnvelope,
                NodeId,
                &mut Effects<'a, S>,
                &'b ActionInput,
            ) -> Result<()>
            + Send
            + Sync
            + 'static,
    {
        let expected_scope = scope_id.as_u128();
        self.register_raw_action(action_id, move |state, envelope, target, effects, input| {
            if input.action_scope_id() == Some(expected_scope) {
                handler(state, envelope, target, effects, input)
            } else {
                Ok(())
            }
        });
    }

    pub fn into_runtime_reducers(self) -> HashMap<ActionId, Vec<BoxedReducer>> {
        let mut runtime_reducers: HashMap<ActionId, Vec<BoxedReducer>> = HashMap::new();
        let state_type_id = TypeId::of::<S>();

        for (action_id, typed_reducers) in self.handlers {
            for typed_reducer in typed_reducers {
                let boxed_reducer: BoxedReducer = Box::new(
                    move |app_states: &mut HashMap<TypeId, Box<dyn AppState>>,
                          action: &ActionEnvelope,
                          target: NodeId,
                          out_effects: &mut Vec<EffectEnvelope>,
                          input: &ActionInput|
                          -> Result<()> {
                        if let Some(state_box) = app_states.get_mut(&state_type_id) {
                            let concrete_state =
                                state_box.downcast_mut::<S>().ok_or_else(|| {
                                    anyhow!("Failed to downcast AppState to concrete type")
                                })?;

                            let mut effects_builder = Effects::new_headless(0);

                            typed_reducer(
                                concrete_state,
                                action,
                                target,
                                &mut effects_builder,
                                input,
                            )?;

                            out_effects.extend(effects_builder.out);

                            Ok(())
                        } else {
                            anyhow::bail!("Target AppState for reducer not found in runtime.");
                        }
                    },
                );

                runtime_reducers
                    .entry(action_id)
                    .or_default()
                    .push(boxed_reducer);
            }
        }
        runtime_reducers
    }
}

/// Identifies which visual property an animation targets.
///
/// Built-in properties have well-known default values (e.g. opacity defaults
/// to 1.0, translation defaults to 0.0). Custom properties use 0.0.
///
/// # Example
///
/// ```rust,ignore
/// ctx.anim_for(widget_id).request(AnimationRequest {
///     property: AnimationPropertyId::Opacity,
///     from: AnimationStartValue::Current,
///     to: 0.0,
///     duration_ms: 300,
///     repeat: false,
///     delay_ms: 0,
/// });
/// ```
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum AnimationPropertyId {
    Opacity,
    TranslateX,
    TranslateY,
    Scale,
    Rotation,
    Custom(Arc<str>),
}

impl AnimationPropertyId {
    pub fn opacity() -> Self {
        Self::Opacity
    }
    pub fn translate_x() -> Self {
        Self::TranslateX
    }
    pub fn translate_y() -> Self {
        Self::TranslateY
    }
    pub fn scale() -> Self {
        Self::Scale
    }
    pub fn rotation() -> Self {
        Self::Rotation
    }
    pub fn custom(name: impl Into<String>) -> Self {
        Self::Custom(Arc::from(name.into()))
    }
    pub fn default_value(&self) -> f32 {
        match self {
            Self::Opacity => 1.0,
            Self::Scale => 1.0,
            Self::TranslateX | Self::TranslateY | Self::Rotation | Self::Custom(_) => 0.0,
        }
    }
}

/// Where an animation starts from.
#[derive(Clone, Debug)]
pub enum AnimationStartValue {
    /// Start from an explicit value.
    Explicit(f32),
    /// Start from whatever the current animated value is.
    Current,
}

/// A request to animate a visual property on a widget.
///
/// Registered via [`BuildCtx::request_animation_for`] or
/// [`AnimCtx::request`].
/// Easing function applied to animation progress before interpolation.
#[derive(Clone, Debug, PartialEq)]
pub enum EasingFunction {
    Linear,
    EaseIn,
    EaseOut,
    EaseInOut,
    CubicBezier(f32, f32, f32, f32),
}

impl Default for EasingFunction {
    fn default() -> Self {
        Self::EaseInOut
    }
}

impl EasingFunction {
    /// Map a linear progress value `t` in [0, 1] to an eased value.
    pub fn apply(&self, t: f32) -> f32 {
        match self {
            Self::Linear => t,
            Self::EaseIn => t * t,
            Self::EaseOut => 1.0 - (1.0 - t) * (1.0 - t),
            Self::EaseInOut => {
                if t < 0.5 {
                    2.0 * t * t
                } else {
                    1.0 - (-2.0 * t + 2.0).powi(2) / 2.0
                }
            }
            Self::CubicBezier(_x1, y1, _x2, y2) => {
                // Simplified cubic bezier approximation (y-axis only)
                let t2 = t * t;
                let t3 = t2 * t;
                3.0 * (1.0 - t) * (1.0 - t) * t * y1 + 3.0 * (1.0 - t) * t2 * y2 + t3
            }
        }
    }
}
#[derive(Clone, Debug)]
pub struct AnimationRequest {
    /// The property to animate.
    pub property: AnimationPropertyId,
    /// Starting value.
    pub from: AnimationStartValue,
    /// Target value.
    pub to: f32,
    /// Duration in milliseconds.
    pub duration_ms: u64,
    /// Whether to loop the animation.
    pub repeat: bool,
    /// Delay before the animation starts (in milliseconds).
    pub delay_ms: u64,
    /// Optional preferred redraw cadence for this animation (in milliseconds).
    ///
    /// This is primarily useful for low-priority repeating effects such as
    /// loading indicators that do not need full frame-rate updates.
    pub frame_interval_ms: Option<u64>,
    pub easing: EasingFunction,
}

/// Registration data for a [`Video`](crate::ui::Video) widget collected during
/// widget building.
#[derive(Clone, Debug)]
pub struct VideoRegistration {
    /// The stable widget identity of the video node.
    pub node_id: WidgetNodeId,
    /// URL or asset path to the video file.
    pub source: String,
    /// Whether to start playing automatically.
    pub autoplay: bool,
    /// Whether to loop playback.
    pub loop_playback: bool,
}

/// Registration data for a platform web view collected during widget building.
#[derive(Clone, Debug)]
pub struct WebRegistration {
    /// The stable widget identity of the web view node.
    pub node_id: WidgetNodeId,
    /// The URL to load.
    pub url: String,
    /// Optional custom user-agent string.
    pub user_agent: Option<String>,
}

/// The mutable context passed to [`Widget::build`](crate::Widget::build).
///
/// `BuildCtx` is where widgets register side-effects that must survive beyond
/// the build phase:
///
/// - **Action binding** -- [`bind`](BuildCtx::bind) registers a handler and
///   returns an [`ActionEnvelope`] that can be stored in widget fields like
///   `on_press`.
/// - **Portals** -- [`register_portal`](BuildCtx::register_portal) places a
///   node in the global overlay stack (modals, toasts, flyouts).
/// - **Animations** -- [`request_animation_for`](BuildCtx::request_animation_for)
///   or the [`anim_for`](BuildCtx::anim_for) helper.
/// - **Video / WebView registration** -- [`register_video`](BuildCtx::register_video),
///   [`register_web_view`](BuildCtx::register_web_view).
///
/// # Example
///
/// ```rust,ignore
/// fn build(&self, ctx: &mut BuildCtx<S>, view: &View<S>) -> Node {
///     let on_press = ctx.bind(MyAction { .. }, reduce_with!(handler));
///     Button { on_press: Some(on_press), ..Default::default() }.into_node()
/// }
/// ```
pub struct BuildCtx<S: AppState> {
    /// The action registry that accumulates handlers during the build phase.
    pub registry: ActionRegistry<S>,
    /// Declarative runtime resources collected during the build phase.
    pub resources: ResourceRegistry,
    /// Pending animation requests.
    pub animation_requests: Vec<(WidgetNodeId, AnimationRequest)>,
    /// Registered video nodes.
    pub video_nodes: Vec<VideoRegistration>,
    /// Registered web view nodes.
    pub web_nodes: Vec<WebRegistration>,
    /// Portal entries (overlays, modals, toasts).
    pub portals: Vec<PortalEntry>,
    portal_seq: u64,
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
/// Created by [`BuildCtx::register_portal`] and friends.
#[derive(Clone, Debug)]
pub struct PortalEntry {
    /// Which overlay layer this portal belongs to.
    pub layer: PortalLayer,
    /// Insertion order (for stable ordering within a layer).
    pub seq: u64,
    /// Optional stable identity.
    pub id: Option<WidgetNodeId>,
    /// The portal's widget tree.
    pub node: Node,
}

impl<S: AppState> BuildCtx<S> {
    pub fn new() -> Self {
        Self {
            registry: ActionRegistry::new(),
            resources: ResourceRegistry::new(),
            animation_requests: Vec::new(),
            video_nodes: Vec::new(),
            web_nodes: Vec::new(),
            portals: Vec::new(),
            portal_seq: 0,
        }
    }

    pub fn bind<A: Action, H>(&mut self, action: A, handler: H) -> ActionEnvelope
    where
        H: IntoHandler<S, A> + Send + Sync + 'static,
    {
        self.registry.register(handler);

        ActionEnvelope {
            id: A::static_id(),
            payload: action.encode(),
        }
    }

    pub fn register<A: Action, H>(&mut self, handler: H)
    where
        H: IntoHandler<S, A> + Send + Sync + 'static,
    {
        self.registry.register::<A, H>(handler);
    }

    pub fn register_scoped_raw_action<F>(
        &mut self,
        scope_id: ActionScopeId,
        action_id: ActionId,
        handler: F,
    ) where
        F: for<'a, 'b> Fn(
                &mut S,
                &ActionEnvelope,
                NodeId,
                &mut Effects<'a, S>,
                &'b ActionInput,
            ) -> Result<()>
            + Send
            + Sync
            + 'static,
    {
        self.registry
            .register_scoped_raw_action(scope_id, action_id, handler);
    }

    pub fn request_animation_for(&mut self, target: WidgetNodeId, request: AnimationRequest) {
        self.animation_requests.push((target, request));
    }

    pub fn register_video(&mut self, registration: VideoRegistration) {
        self.video_nodes.push(registration);
    }

    pub fn register_web_view(&mut self, registration: WebRegistration) {
        self.web_nodes.push(registration);
    }

    pub fn take_animation_requests(&mut self) -> Vec<(WidgetNodeId, AnimationRequest)> {
        std::mem::take(&mut self.animation_requests)
    }

    pub fn take_video_registrations(&mut self) -> Vec<VideoRegistration> {
        std::mem::take(&mut self.video_nodes)
    }

    pub fn take_web_registrations(&mut self) -> Vec<WebRegistration> {
        std::mem::take(&mut self.web_nodes)
    }

    pub fn take_resources(&mut self) -> Vec<RuntimeResourceDeclaration> {
        self.resources.take()
    }

    pub fn register_portal(&mut self, node: Node) {
        self.register_portal_with_layer(PortalLayer::Default, None, node);
    }

    pub fn register_portal_with_id(&mut self, id: WidgetNodeId, node: Node) {
        self.register_portal_with_layer(PortalLayer::Default, Some(id), node);
    }

    pub fn register_portal_with_layer(
        &mut self,
        layer: PortalLayer,
        id: Option<WidgetNodeId>,
        node: Node,
    ) {
        let seq = self.portal_seq;
        self.portal_seq = self.portal_seq.wrapping_add(1);
        self.portals.push(PortalEntry {
            layer,
            seq,
            id,
            node,
        });
    }

    pub fn take_portals(&mut self) -> Vec<(Option<WidgetNodeId>, Node)> {
        let mut entries = std::mem::take(&mut self.portals);
        entries.sort_by(|a, b| (a.layer, a.seq).cmp(&(b.layer, b.seq)));
        entries.into_iter().map(|e| (e.id, e.node)).collect()
    }

    pub fn anim_for(&mut self, target: WidgetNodeId) -> AnimCtx<'_, S> {
        AnimCtx { target, ctx: self }
    }

    pub fn video_controls(&self, target: WidgetNodeId) -> VideoControlCtx {
        VideoControlCtx { target }
    }
}

pub struct AnimCtx<'a, S: AppState> {
    target: WidgetNodeId,
    ctx: &'a mut BuildCtx<S>,
}

impl<'a, S: AppState> AnimCtx<'a, S> {
    pub fn request(&mut self, request: AnimationRequest) {
        self.ctx.request_animation_for(self.target, request);
    }

    pub fn request_for(&mut self, target: WidgetNodeId, request: AnimationRequest) {
        self.ctx.request_animation_for(target, request);
    }
}

#[derive(Clone, Copy)]
pub struct VideoControlCtx {
    target: WidgetNodeId,
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

    pub fn widget(name: impl AsRef<str>, id: WidgetNodeId) -> Self {
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
