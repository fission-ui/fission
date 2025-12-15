use crate::{Action, ActionEnvelope, ActionId, AppState, BoxedReducer};
use anyhow::{anyhow, Result};
use fission_ir::{NodeId, WidgetNodeId};
use serde::{Deserialize, Serialize};
use std::any::TypeId;
use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

pub type Handler<S, A> = fn(&mut S, A);

// We store a factory that can create the runtime-compatible reducer.
// Or we just store the typed closure logic.
type TypedReducer<S> = Box<dyn Fn(&mut S, &ActionEnvelope) -> Result<()> + Send + Sync>;

pub struct ActionRegistry<S: AppState> {
    handlers: BTreeMap<ActionId, TypedReducer<S>>,
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

    pub fn register<A: Action>(&mut self, handler: Handler<S, A>) {
        let action_id = A::static_id();

        let typed_reducer = Box::new(
            move |state: &mut S, envelope: &ActionEnvelope| -> Result<()> {
                let action: A = serde_json::from_slice(&envelope.payload)
                    .map_err(|e| anyhow!("Failed to deserialize action: {}", e))?;
                handler(state, action);
                Ok(())
            },
        );

        self.handlers.insert(action_id, typed_reducer);
    }

    // Convert this registry into the format Runtime expects
    pub fn into_runtime_reducers(self) -> HashMap<ActionId, Vec<BoxedReducer>> {
        let mut runtime_reducers: HashMap<ActionId, Vec<BoxedReducer>> = HashMap::new();
        let state_type_id = TypeId::of::<S>();

        for (action_id, typed_reducer) in self.handlers {
            // Wrap the typed_reducer into a BoxedReducer that looks up S from the Runtime's state map
            let boxed_reducer: BoxedReducer = Box::new(
                move |app_states: &mut HashMap<TypeId, Box<dyn AppState>>,
                      action: &ActionEnvelope,
                      _target: NodeId|
                      -> Result<()> {
                    if let Some(state_box) = app_states.get_mut(&state_type_id) {
                        let concrete_state = state_box.downcast_mut::<S>().ok_or_else(|| {
                            anyhow!("Failed to downcast AppState to concrete type")
                        })?;
                        typed_reducer(concrete_state, action)
                    } else {
                        // If the state isn't present, we can't run this reducer.
                        // Should we error? Yes.
                        anyhow::bail!("Target AppState for reducer not found in runtime.");
                    }
                },
            );

            runtime_reducers
                .entry(action_id)
                .or_default()
                .push(boxed_reducer);
        }
        runtime_reducers
    }
}

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

#[derive(Clone, Debug)]
pub enum AnimationStartValue {
    Explicit(f32),
    Current,
}

#[derive(Clone, Debug)]
pub struct AnimationRequest {
    pub property: AnimationPropertyId,
    pub from: AnimationStartValue,
    pub to: f32,
    pub duration_ms: u64,
}

#[derive(Clone, Debug)]
pub struct VideoRegistration {
    pub node_id: WidgetNodeId,
    pub source: String,
    pub autoplay: bool,
    pub loop_playback: bool,
}

pub struct BuildCtx<S: AppState> {
    pub registry: ActionRegistry<S>,
    pub animation_requests: Vec<(WidgetNodeId, AnimationRequest)>,
    pub video_nodes: Vec<VideoRegistration>,
}

impl<S: AppState> BuildCtx<S> {
    pub fn new() -> Self {
        Self {
            registry: ActionRegistry::new(),
            animation_requests: Vec::new(),
            video_nodes: Vec::new(),
        }
    }

    pub fn bind<A: Action>(&mut self, action: A, handler: Handler<S, A>) -> ActionEnvelope {
        self.registry.register(handler);

        ActionEnvelope {
            id: A::static_id(),
            payload: action.encode(),
        }
    }

    pub fn request_animation_for(&mut self, target: WidgetNodeId, request: AnimationRequest) {
        self.animation_requests.push((target, request));
    }

    pub fn register_video(&mut self, registration: VideoRegistration) {
        self.video_nodes.push(registration);
    }

    pub fn take_animation_requests(&mut self) -> Vec<(WidgetNodeId, AnimationRequest)> {
        std::mem::take(&mut self.animation_requests)
    }

    pub fn take_video_registrations(&mut self) -> Vec<VideoRegistration> {
        std::mem::take(&mut self.video_nodes)
    }

    pub fn anim_for(&mut self, target: WidgetNodeId) -> AnimCtx<'_, S> {
        AnimCtx { target, ctx: self }
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
