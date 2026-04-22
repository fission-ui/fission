use crate::{
    action::video::{
        VideoPause, VideoPlay, VideoSeek, VideoSetMuted, VideoSetRate, VideoSetVolume, VideoStop,
    },
    Action, ActionEnvelope, ActionId, AppState, BoxedReducer,
    ui::Node,
    context::{Effects, ReducerContext},
    effect::{EffectEnvelope, ActionInput},
};
use anyhow::{anyhow, Result};
use fission_ir::{NodeId, WidgetNodeId};
use serde::{Deserialize, Serialize};
use std::any::TypeId;
use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

// The canonical handler signature
pub type Handler<S, A> = for<'a, 'b, 'c> fn(&mut S, A, &mut ReducerContext<'a, 'b, 'c, S>);

// The trait for backward compatibility
pub trait IntoHandler<S: AppState, A> {
    fn call<'a, 'b, 'c>(&self, state: &mut S, action: A, ctx: &mut ReducerContext<'a, 'b, 'c, S>);
}

// Impl for Legacy (2-arg)
impl<S: AppState, A> IntoHandler<S, A> for fn(&mut S, A) {
    fn call<'a, 'b, 'c>(&self, state: &mut S, action: A, _ctx: &mut ReducerContext<'a, 'b, 'c, S>) {
        (self)(state, action);
    }
}

// Impl for Modern (3-arg)
impl<S: AppState, A> IntoHandler<S, A> for for<'a, 'b, 'c> fn(&mut S, A, &mut ReducerContext<'a, 'b, 'c, S>) {
    fn call<'a, 'b, 'c>(&self, state: &mut S, action: A, ctx: &mut ReducerContext<'a, 'b, 'c, S>) {
        (self)(state, action, ctx);
    }
}

// Internal typed reducer storage
type TypedReducer<S> = Box<dyn for<'a, 'b, 'c> Fn(&mut S, &ActionEnvelope, &mut Effects<'a, S>, &'b ActionInput) -> Result<()> + Send + Sync>;

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

    pub fn register<A: Action, H: IntoHandler<S, A> + Send + Sync + 'static>(&mut self, handler: H) {
        let action_id = A::static_id();

        let typed_reducer: TypedReducer<S> = Box::new(
            move |state: &mut S, envelope: &ActionEnvelope, effects, input| -> Result<()> {
                let action: A = serde_json::from_slice(&envelope.payload)
                    .map_err(|e| anyhow!("Failed to deserialize action: {}", e))?;
                
                let mut ctx = ReducerContext {
                    effects,
                    input,
                };
                
                handler.call(state, action, &mut ctx);
                Ok(())
            },
        );

        self.handlers.insert(action_id, typed_reducer);
    }

    pub fn into_runtime_reducers(self) -> HashMap<ActionId, Vec<BoxedReducer>> {
        let mut runtime_reducers: HashMap<ActionId, Vec<BoxedReducer>> = HashMap::new();
        let state_type_id = TypeId::of::<S>();

        for (action_id, typed_reducer) in self.handlers {
            let boxed_reducer: BoxedReducer = Box::new(
                move |app_states: &mut HashMap<TypeId, Box<dyn AppState>>,
                      action: &ActionEnvelope,
                      _target: NodeId,
                      out_effects: &mut Vec<EffectEnvelope>,
                      input: &ActionInput|
                      -> Result<()> {
                    if let Some(state_box) = app_states.get_mut(&state_type_id) {
                        let concrete_state = state_box.downcast_mut::<S>().ok_or_else(|| {
                            anyhow!("Failed to downcast AppState to concrete type")
                        })?;
                        
                        let mut effects_builder = Effects::new_headless(0); 
                        
                        typed_reducer(concrete_state, action, &mut effects_builder, input)?;
                        
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
        runtime_reducers
    }
}

// ... Rest of file same ...
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
    pub fn opacity() -> Self { Self::Opacity }
    pub fn translate_x() -> Self { Self::TranslateX }
    pub fn translate_y() -> Self { Self::TranslateY }
    pub fn scale() -> Self { Self::Scale }
    pub fn rotation() -> Self { Self::Rotation }
    pub fn custom(name: impl Into<String>) -> Self { Self::Custom(Arc::from(name.into())) }
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
    pub repeat: bool,
    pub delay_ms: u64,
}

#[derive(Clone, Debug)]
pub struct VideoRegistration {
    pub node_id: WidgetNodeId,
    pub source: String,
    pub autoplay: bool,
    pub loop_playback: bool,
}

#[derive(Clone, Debug)]
pub struct WebRegistration {
    pub node_id: WidgetNodeId,
    pub url: String,
    pub user_agent: Option<String>,
}

pub struct BuildCtx<S: AppState> {
    pub registry: ActionRegistry<S>,
    pub animation_requests: Vec<(WidgetNodeId, AnimationRequest)>,
    pub video_nodes: Vec<VideoRegistration>,
    pub web_nodes: Vec<WebRegistration>,
    pub portals: Vec<PortalEntry>,
    portal_seq: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum PortalLayer {
    Default = 0,
    Modal = 100,
    Flyout = 200,
    Toast = 300,
}

#[derive(Clone, Debug)]
pub struct PortalEntry {
    pub layer: PortalLayer,
    pub seq: u64,
    pub id: Option<WidgetNodeId>,
    pub node: Node,
}

impl<S: AppState> BuildCtx<S> {
    pub fn new() -> Self {
        Self {
            registry: ActionRegistry::new(),
            animation_requests: Vec::new(),
            video_nodes: Vec::new(),
            web_nodes: Vec::new(),
            portals: Vec::new(),
            portal_seq: 0,
        }
    }

    pub fn bind<A: Action, H>(&mut self, action: A, handler: H) -> ActionEnvelope 
    where H: IntoHandler<S, A> + Send + Sync + 'static 
    {
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

    pub fn register_portal(&mut self, node: Node) {
        self.register_portal_with_layer(PortalLayer::Default, None, node);
    }

    pub fn register_portal_with_id(&mut self, id: WidgetNodeId, node: Node) {
        self.register_portal_with_layer(PortalLayer::Default, Some(id), node);
    }

    pub fn register_portal_with_layer(&mut self, layer: PortalLayer, id: Option<WidgetNodeId>, node: Node) {
        let seq = self.portal_seq;
        self.portal_seq = self.portal_seq.wrapping_add(1);
        self.portals.push(PortalEntry { layer, seq, id, node });
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
