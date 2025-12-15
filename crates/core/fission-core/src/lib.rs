use anyhow::{anyhow, Result};
use downcast_rs::Downcast;
use fission_ir::CoreIR;
use lazy_static::lazy_static;
use std::any::{Any, TypeId};
use std::collections::{HashMap, HashSet};

pub mod action;
pub mod diff;
pub mod env;
pub mod event;
pub mod hit_test;
pub mod lowering;
pub mod registry;
pub mod time;
pub mod ui;
pub mod view;

use crate::action::video::{VideoPause, VideoPlay};
use crate::env::ActiveAnimation;
pub use action::{Action, ActionEnvelope, ActionId, AppState};
pub use env::{Env, InteractionStateMap, RuntimeState, ScrollStateMap};
pub use event::{InputEvent, KeyCode, KeyEvent, LifecycleEvent, PointerButton, PointerEvent};
pub use fission_ir::op;
pub use fission_ir::{EmbedKind, NodeId, Op, WidgetNodeId};
pub use fission_layout::{
    FlexDirection, LayoutOp, LayoutPoint, LayoutRect, LayoutSize, LayoutSnapshot, LayoutUnit,
};
use hit_test::{find_next_focus_node, hit_test, hit_test_with_scroll};
pub use lowering::{LoweringContext, NodeBuilder};
pub use registry::{
    ActionRegistry, AnimationPropertyId, AnimationRequest, AnimationStartValue, BuildCtx, Handler,
    VideoRegistration,
};
pub use time::{Clock, CurrentTime};
pub use ui::{Button, Column, CustomNode, Lower, LowerDyn, Node, Row, Text};
pub use view::{Selector, View, Widget};

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Tick {
    pub dt: CurrentTime,
}

impl Action for Tick {
    fn static_id() -> ActionId {
        *TICK_ACTION_ID
    }
}

lazy_static! {
    pub static ref TICK_ACTION_ID: ActionId = ActionId::from_name("fission_core::Tick");
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct AdvanceTo {
    pub time: CurrentTime,
}

impl Action for AdvanceTo {
    fn static_id() -> ActionId {
        *ADVANCE_TO_ACTION_ID
    }
}

lazy_static! {
    pub static ref ADVANCE_TO_ACTION_ID: ActionId = ActionId::from_name("fission_core::AdvanceTo");
}

pub type BoxedReducer = Box<
    dyn FnMut(&mut HashMap<TypeId, Box<dyn AppState>>, &ActionEnvelope, NodeId) -> Result<()>
        + Send
        + Sync,
>;

pub struct Runtime {
    reducers: HashMap<ActionId, Vec<BoxedReducer>>,
    app_states: HashMap<TypeId, Box<dyn AppState>>,
    pub runtime_state: RuntimeState,
}

impl Default for Runtime {
    fn default() -> Self {
        let mut runtime = Self {
            reducers: HashMap::new(),
            app_states: HashMap::new(),
            runtime_state: RuntimeState::default(),
        };

        runtime
            .add_app_state(Box::new(Clock::default()))
            .expect("Failed to add Clock state");
        // runtime.add_app_state(Box::new(VideoStateMap::default())).expect("Failed to add VideoStateMap");

        runtime.register_base_reducers();

        runtime
    }
}

impl Runtime {
    pub fn register_base_reducers(&mut self) {
        self.register_reducer::<Clock>(
            *TICK_ACTION_ID,
            |state: &mut Clock, action: &ActionEnvelope, _target| {
                let tick_action: Tick = serde_json::from_slice(&action.payload)
                    .map_err(|e| anyhow!("Failed to deserialize Tick: {}", e))?;
                state.advance_by(tick_action.dt)
            },
        )
        .expect("Failed to register Tick reducer");

        self.register_reducer::<Clock>(
            *ADVANCE_TO_ACTION_ID,
            |state: &mut Clock, action: &ActionEnvelope, _target| {
                let advance_action: AdvanceTo = serde_json::from_slice(&action.payload)
                    .map_err(|e| anyhow!("Failed to deserialize AdvanceTo: {}", e))?;
                state.set_to(advance_action.time)
            },
        )
        .expect("Failed to register AdvanceTo reducer");

        // self.register_reducer::<VideoStateMap>(*VIDEO_PLAY_ID, |state: &mut VideoStateMap, _action: &ActionEnvelope, target| {
        //     if let Some(video_state) = state.states.get_mut(&target) {
        //         video_state.status = VideoStatus::Playing;
        //     }
        //     Ok(())
        // }).expect("Failed to register VideoPlay reducer");

        // self.register_reducer::<VideoStateMap>(*VIDEO_PAUSE_ID, |state: &mut VideoStateMap, _action: &ActionEnvelope, target| {
        //     if let Some(video_state) = state.states.get_mut(&target) {
        //         video_state.status = VideoStatus::Paused;
        //     }
        //     Ok(())
        // }).expect("Failed to register VideoPause reducer");
    }

    pub fn clear_reducers(&mut self) {
        self.reducers.clear();
        self.register_base_reducers();
    }

    pub fn absorb_registry<S: AppState>(&mut self, registry: ActionRegistry<S>) {
        let new_reducers = registry.into_runtime_reducers();
        for (id, mut list) in new_reducers {
            self.reducers.entry(id).or_default().append(&mut list);
        }
    }

    pub fn clock(&self) -> &Clock {
        self.get_app_state::<Clock>()
            .expect("Clock state must always be present")
    }

    pub fn get_app_state<S: AppState + 'static>(&self) -> Option<&S> {
        self.app_states
            .get(&TypeId::of::<S>())
            .and_then(|s_box| s_box.downcast_ref::<S>())
    }

    pub fn get_app_state_mut<S: AppState + 'static>(&mut self) -> Option<&mut S> {
        self.app_states
            .get_mut(&TypeId::of::<S>())
            .and_then(|s_box| s_box.downcast_mut::<S>())
    }

    pub fn add_app_state<S: AppState + 'static>(&mut self, state: Box<S>) -> Result<()> {
        let type_id = TypeId::of::<S>();
        if self.app_states.insert(type_id, state).is_some() {
            anyhow::bail!("App state of this type already registered.");
        }
        Ok(())
    }

    pub fn register_reducer<S: AppState + 'static>(
        &mut self,
        action_id: ActionId,
        reducer_fn: fn(&mut S, &ActionEnvelope, NodeId) -> Result<()>,
    ) -> Result<()> {
        let state_type_id = TypeId::of::<S>();

        let boxed_reducer: BoxedReducer = Box::new(
            move |app_states: &mut HashMap<TypeId, Box<dyn AppState>>,
                  action: &ActionEnvelope,
                  target: NodeId|
                  -> Result<()> {
                if let Some(state_box) = app_states.get_mut(&state_type_id) {
                    let concrete_state = state_box.downcast_mut::<S>().ok_or_else(|| {
                        anyhow!("Failed to downcast AppState to concrete type for reducer")
                    })?;
                    reducer_fn(concrete_state, action, target)
                } else {
                    anyhow::bail!("Target AppState for reducer not found in runtime.");
                }
            },
        );

        self.reducers
            .entry(action_id)
            .or_default()
            .push(boxed_reducer);
        Ok(())
    }

    pub fn dispatch(&mut self, action: ActionEnvelope, target: NodeId) -> Result<()> {
        if action.id == VideoPlay::static_id() {
            let cmd: VideoPlay = serde_json::from_slice(&action.payload)
                .map_err(|e| anyhow!("Failed to deserialize VideoPlay: {}", e))?;
            if let Some(video_state) = self.runtime_state.video.states.get_mut(&cmd.target) {
                video_state.status = env::VideoStatus::Playing;
            }
            return Ok(());
        }

        if action.id == VideoPause::static_id() {
            let cmd: VideoPause = serde_json::from_slice(&action.payload)
                .map_err(|e| anyhow!("Failed to deserialize VideoPause: {}", e))?;
            if let Some(video_state) = self.runtime_state.video.states.get_mut(&cmd.target) {
                video_state.status = env::VideoStatus::Paused;
            }
            return Ok(());
        }

        // System Actions - Removed Animate dispatch

        let action_id = action.id;
        if let Some(reducers) = self.reducers.get_mut(&action_id) {
            let mut temp_reducers: Vec<BoxedReducer> = reducers.drain(..).collect();

            for reducer_wrapper in temp_reducers.iter_mut() {
                reducer_wrapper(&mut self.app_states, &action, target)?;
            }
            reducers.extend(temp_reducers);
        }
        Ok(())
    }

    pub fn tick(&mut self, dt: CurrentTime) -> Result<()> {
        let action = Tick { dt };
        let envelope: ActionEnvelope = action.into();
        self.dispatch(envelope, NodeId::derived(0, &[0]))?;

        let current_time = self.clock().current_time();

        let mut finished = Vec::new();
        for ((target, property), anim) in self.runtime_state.animation.active.iter_mut() {
            let elapsed = current_time.saturating_sub(anim.start_time);
            let progress = if anim.duration == 0 {
                1.0
            } else {
                (elapsed as f32 / anim.duration as f32).clamp(0.0, 1.0)
            };
            let value = anim.start_value + (anim.end_value - anim.start_value) * progress;
            self.runtime_state
                .animation
                .values
                .insert((*target, property.clone()), value);
            if progress >= 1.0 {
                finished.push((*target, property.clone()));
            }
        }

        for key in finished {
            self.runtime_state.animation.active.remove(&key);
        }

        Ok(())
    }

    pub fn enqueue_animation(&mut self, target: WidgetNodeId, request: AnimationRequest) {
        let key = (target, request.property.clone());
        let current_value = self
            .runtime_state
            .animation
            .values
            .get(&key)
            .copied()
            .unwrap_or_else(|| request.property.default_value());
        let start_value = match request.from {
            AnimationStartValue::Explicit(v) => v,
            AnimationStartValue::Current => current_value,
        };

        let anim = ActiveAnimation {
            target,
            property: request.property.clone(),
            start_value,
            end_value: request.to,
            start_time: self.clock().current_time(),
            duration: request.duration_ms,
        };

        self.runtime_state
            .animation
            .values
            .insert(key.clone(), start_value);
        self.runtime_state.animation.active.insert(key, anim);
    }

    pub fn sync_video_nodes(&mut self, registrations: &[VideoRegistration]) {
        let mut seen: HashSet<WidgetNodeId> = HashSet::new();

        for reg in registrations {
            seen.insert(reg.node_id);
            let entry = self
                .runtime_state
                .video
                .states
                .entry(reg.node_id)
                .or_insert_with(env::VideoState::default);
            entry.asset_source = reg.source.clone();
            entry.looped = reg.loop_playback;
            if reg.autoplay && entry.status == env::VideoStatus::Stopped {
                entry.status = env::VideoStatus::Playing;
            }
        }

        self.runtime_state
            .video
            .states
            .retain(|node_id, _| seen.contains(node_id));
    }

    pub fn handle_input(
        &mut self,
        event: InputEvent,
        ir: &CoreIR,
        layout: &LayoutSnapshot,
    ) -> Result<()> {
        match event {
            InputEvent::Pointer(PointerEvent::Scroll { point, delta }) => {
                if let Some(hit_node_id) =
                    hit_test_with_scroll(ir, layout, &self.runtime_state.scroll, point)
                {
                    let mut current_id = Some(hit_node_id);
                    while let Some(node_id) = current_id {
                        if let Some(node) = ir.nodes.get(&node_id) {
                            if let Op::Layout(op::LayoutOp::Scroll { .. }) = &node.op {
                                let current_offset = self.runtime_state.scroll.get_offset(node_id);
                                let mut new_offset = current_offset + delta.y; // Assuming vertical scroll

                                // Clamping logic
                                if let Some(geom) = layout.get_node_geometry(node_id) {
                                    let max_offset =
                                        (geom.content_size.height - geom.rect.height()).max(0.0);
                                    new_offset = new_offset.clamp(0.0, max_offset);
                                }

                                self.runtime_state.scroll.set_offset(node_id, new_offset);
                                break;
                            }
                            current_id = node.parent;
                        } else {
                            break;
                        }
                    }
                }
            }
            InputEvent::Keyboard(KeyEvent::Down {
                key_code,
                modifiers,
            }) => match key_code {
                KeyCode::Tab => {
                    let reverse = (modifiers & 1) != 0;
                    let next =
                        find_next_focus_node(ir, self.runtime_state.interaction.focused, reverse);
                    self.runtime_state.interaction.set_focused(next);
                }
                KeyCode::Enter | KeyCode::Space => {
                    if let Some(focused_id) = self.runtime_state.interaction.focused {
                        let mut current_id = Some(focused_id);
                        while let Some(node_id) = current_id {
                            if let Some(node) = ir.nodes.get(&node_id) {
                                if let Op::Semantics(semantics) = &node.op {
                                    if let Some(action_entry) = semantics.actions.entries.first() {
                                        if let Some(payload) = &action_entry.payload_data {
                                            let envelope = ActionEnvelope {
                                                id: ActionId::from_u128(action_entry.action_id),
                                                payload: payload.clone(),
                                            };
                                            return self.dispatch(envelope, node_id);
                                        }
                                    }
                                }
                                current_id = node.parent;
                            } else {
                                break;
                            }
                        }
                    }
                }
                _ => {}
            },
            InputEvent::Pointer(PointerEvent::Move { point, .. }) => {
                let hit = hit_test_with_scroll(ir, layout, &self.runtime_state.scroll, point);

                let mut new_hovered = std::collections::HashSet::new();
                if let Some(mut node_id) = hit {
                    loop {
                        new_hovered.insert(node_id);
                        if let Some(node) = ir.nodes.get(&node_id) {
                            if let Some(parent) = node.parent {
                                node_id = parent;
                            } else {
                                break;
                            }
                        } else {
                            break;
                        }
                    }
                }

                self.runtime_state.interaction.hovered.clear();

                for id in new_hovered {
                    self.runtime_state.interaction.set_hovered(id, true);
                }
            }
            InputEvent::Pointer(PointerEvent::Down { point, .. }) => {
                if let Some(hit_node_id) =
                    hit_test_with_scroll(ir, layout, &self.runtime_state.scroll, point)
                {
                    let mut focus_candidate = Some(hit_node_id);
                    while let Some(node_id) = focus_candidate {
                        if let Some(node) = ir.nodes.get(&node_id) {
                            if let Op::Semantics(s) = &node.op {
                                if s.focusable {
                                    self.runtime_state.interaction.set_focused(Some(node_id));
                                    break;
                                }
                            }
                            focus_candidate = node.parent;
                        } else {
                            break;
                        }
                    }
                    if focus_candidate.is_none() {
                        // self.runtime_state.interaction.set_focused(None);
                    }

                    let mut current_pressed_id = Some(hit_node_id);
                    while let Some(node_id) = current_pressed_id {
                        self.runtime_state.interaction.set_pressed(node_id, true);
                        if let Some(node) = ir.nodes.get(&node_id) {
                            current_pressed_id = node.parent;
                        } else {
                            break;
                        }
                    }

                    let mut current_id = Some(hit_node_id);
                    while let Some(node_id) = current_id {
                        if let Some(node) = ir.nodes.get(&node_id) {
                            if let Op::Semantics(semantics) = &node.op {
                                if let Some(action_entry) = semantics.actions.entries.first() {
                                    if let Some(payload) = &action_entry.payload_data {
                                        let envelope = ActionEnvelope {
                                            id: ActionId::from_u128(action_entry.action_id),
                                            payload: payload.clone(),
                                        };
                                        println!("Dispatching action {:?} from input", envelope.id);
                                        return self.dispatch(envelope, node_id);
                                    } else {
                                        println!("ActionEntry found but no payload data.");
                                    }
                                }
                            }

                            current_id = node.parent;
                        } else {
                            break;
                        }
                    }
                } else {
                    self.runtime_state.interaction.set_focused(None);
                }
            }
            InputEvent::Pointer(PointerEvent::Up { point: _, .. }) => {
                self.runtime_state.interaction.pressed.clear();
            }
            _ => {}
        }
        Ok(())
    }
}
