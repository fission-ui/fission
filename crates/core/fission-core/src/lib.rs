use anyhow::{anyhow, Result};
use fission_diagnostics::prelude as diag;
use downcast_rs::Downcast;
use fission_ir::CoreIR;
use lazy_static::lazy_static;
use serde_json;
use std::any::{Any, TypeId};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

extern crate self as fission_core;

pub mod action;
pub mod diff;
pub mod env;
pub mod event;
pub mod hit_test;
pub mod input;
pub mod lowering;
pub mod media; // New module
pub mod registry;
pub mod time;
pub mod ui;
pub mod view;

use crate::env::ActiveAnimation;
pub use action::{Action, ActionEnvelope, ActionId, AppState};
pub use env::{Env, InteractionStateMap, RuntimeState, ScrollStateMap, Clipboard, ImeHandler};

pub use event::{InputEvent, KeyCode, KeyEvent, LifecycleEvent, PointerButton, PointerEvent};
pub use fission_ir::op;
pub use fission_ir::{EmbedKind, NodeId, Op, WidgetNodeId};
pub use fission_layout::{
    FlexDirection, LayoutOp, LayoutPoint, LayoutRect, LayoutSize, LayoutSnapshot, LayoutUnit, TextMeasurer,
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
    pub measurer: Option<Arc<dyn TextMeasurer>>,
    pub clipboard_backend: Option<Arc<dyn Clipboard>>,
    pub ime_handler: Option<Arc<dyn ImeHandler>>,
}

impl Default for Runtime {
    fn default() -> Self {
        let mut runtime = Self {
            reducers: HashMap::new(),
            app_states: HashMap::new(),
            runtime_state: RuntimeState::default(),
            measurer: None,
            clipboard_backend: None,
            ime_handler: None,
        };

        runtime
            .add_app_state(Box::new(Clock::default()))
            .expect("Failed to add Clock state");

        runtime.register_base_reducers();

        runtime
    }
}

impl Runtime {
    pub fn with_measurer(mut self, measurer: Arc<dyn TextMeasurer>) -> Self {
        self.measurer = Some(measurer);
        self
    }

    pub fn with_clipboard(mut self, backend: Arc<dyn Clipboard>) -> Self {
        self.clipboard_backend = Some(backend);
        self
    }

    pub fn with_ime_handler(mut self, handler: Arc<dyn ImeHandler>) -> Self {
        self.ime_handler = Some(handler);
        self
    }

    pub fn caret_from_point_in_text(&self, value: &str, font_size: f32, viewport_x: f32, viewport_w: f32, content_w: f32, scroll_offset: f32, point_x: f32) -> usize {
        // Delegate to static helper in input module
        crate::input::text::caret_from_point_in_text(
            self.measurer.as_ref(),
            value,
            font_size,
            viewport_x,
            viewport_w,
            content_w,
            scroll_offset,
            point_x
        )
    }

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
        diag::emit(
            diag::DiagCategory::Input,
            diag::DiagLevel::Debug,
            diag::DiagEventKind::InputEvent { kind: "dispatch_start".into(), target: Some(target.as_u128()), position: None },
        );
        
        // Delegate video actions to media module
        if crate::media::handle_video_action(&mut self.runtime_state.video, &action)? {
            return Ok(());
        }

        let action_id = action.id;
        if let Some(reducers) = self.reducers.get_mut(&action_id) {
            diag::emit(
                diag::DiagCategory::Input,
                diag::DiagLevel::Debug,
                diag::DiagEventKind::InputEvent { kind: format!("reducers:{}", reducers.len()), target: Some(target.as_u128()), position: None },
            );
            let mut temp_reducers: Vec<BoxedReducer> = reducers.drain(..).collect();

            for reducer_wrapper in temp_reducers.iter_mut() {
                reducer_wrapper(&mut self.app_states, &action, target)?;
            }
            reducers.extend(temp_reducers);
        }
        diag::emit(
            diag::DiagCategory::Input,
            diag::DiagLevel::Debug,
            diag::DiagEventKind::InputEvent { kind: "dispatch_end".into(), target: Some(target.as_u128()), position: None },
        );
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
        use crate::input::{ControllerContext, InputController};
        use crate::input::text::TextInputController;

        let mut dispatched_actions = Vec::new();
        let mut handled = false;

        {
            let mut ctx = ControllerContext {
                ir,
                layout,
                text_edit: &mut self.runtime_state.text_edit,
                interaction: &mut self.runtime_state.interaction,
                scroll: &mut self.runtime_state.scroll,
                ime_preedit: &mut self.runtime_state.ime_preedit,
                clipboard: self.clipboard_backend.as_ref(), 
                measurer: self.measurer.as_ref(),
                dispatched_actions: Vec::new(),
            };

            let mut controller = TextInputController;
            if controller.handle_event(&mut ctx, &event) {
                handled = true;
                dispatched_actions = ctx.dispatched_actions;
            }
        }

        for (target, action) in dispatched_actions {
            self.dispatch(action, target)?;
        }

        if handled {
            return Ok(());
        }

        match event {
            InputEvent::Pointer(PointerEvent::Scroll { point, delta }) => {
                if let Some(hit_node_id) =
                    hit_test_with_scroll(ir, layout, &self.runtime_state.scroll, point)
                {
                    let mut current_id = Some(hit_node_id);
                    while let Some(node_id) = current_id {
                        if let Some(node) = ir.nodes.get(&node_id) {
                            if let Op::Layout(op::LayoutOp::Scroll { direction, .. }) = &node.op {
                                let current_offset = self.runtime_state.scroll.get_offset(node_id);
                                let delta_val = match direction { op::FlexDirection::Row => delta.x, op::FlexDirection::Column => delta.y };
                                let mut new_offset = current_offset + delta_val;

                                // Clamping logic
                                if let Some(geom) = layout.get_node_geometry(node_id) {
                                    let max_offset = if matches!(direction, op::FlexDirection::Row) {
                                        (geom.content_size.width - geom.rect.width()).max(0.0)
                                    } else {
                                        (geom.content_size.height - geom.rect.height()).max(0.0)
                                    };
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
                    let next = find_next_focus_node(ir, self.runtime_state.interaction.focused, reverse);
                    if next != self.runtime_state.interaction.focused {
                        self.runtime_state.ime_preedit = None;
                    }
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
            InputEvent::Pointer(PointerEvent::Down { point, .. }) => {
                if let Some(hit_node_id) =
                    hit_test_with_scroll(ir, layout, &self.runtime_state.scroll, point)
                {
                    diag::emit(
                        diag::DiagCategory::Input,
                        diag::DiagLevel::Debug,
                        diag::DiagEventKind::InputEvent { kind: "pointer_down_hit".into(), target: Some(hit_node_id.as_u128()), position: Some((point.x, point.y)) },
                    );
                    let mut focus_candidate = Some(hit_node_id);
                    while let Some(node_id) = focus_candidate {
                        if let Some(node) = ir.nodes.get(&node_id) {
                            if let Op::Semantics(s) = &node.op {
                                if s.focusable {
                                    let old_focused_id = self.runtime_state.interaction.focused;
                                    if Some(node_id) != old_focused_id {
                                        // Clear preedit if focus changed
                                        self.runtime_state.ime_preedit = None;

                                        // Activate IME if new focus is a TextInput
                                        if s.role == fission_ir::semantics::Role::TextInput {
                                            if let Some(ime_handler) = &self.ime_handler {
                                                ime_handler.set_ime_allowed(true);
                                                // TODO: Set actual caret rect here (requires layout information, which is hard in handle_input context without a current layout)
                                            }
                                        } else if let Some(ime_handler) = &self.ime_handler {
                                            // Deactivate IME if focus is not a TextInput
                                            ime_handler.set_ime_allowed(false);
                                        }
                                    }
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
                        // No focusable element hit.
                        // If a TextInput was previously focused, deactivate IME
                        if let Some(old_focused_id) = self.runtime_state.interaction.focused {
                            if let Some(old_node) = ir.nodes.get(&old_focused_id) {
                                if let Op::Semantics(s) = &old_node.op {
                                    if s.role == fission_ir::semantics::Role::TextInput {
                                        if let Some(ime_handler) = &self.ime_handler {
                                            ime_handler.set_ime_allowed(false);
                                        }
                                    }
                                }
                            }
                        }
                        self.runtime_state.interaction.set_focused(None);
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
                    // Record pointer down location (for move threshold)
                    self.runtime_state.interaction.last_down_point = Some(point);

                    // Update caret rect for IME if TextInput is focused
                    if let Some(focused_id) = self.runtime_state.interaction.focused {
                        if let Some(node) = ir.nodes.get(&focused_id) {
                            if let Op::Semantics(s) = &node.op {
                                if s.role == fission_ir::semantics::Role::TextInput {
                                    // After a click, update caret rect for IME
                                    if let Some(ime_handler) = &self.ime_handler {
                                        // This is a dummy rect for now, actual caret position would be here after layout.
                                        ime_handler.set_ime_cursor_area(LayoutRect::new(point.x, point.y, 2.0, 16.0));
                                    }
                                }
                            }
                        }
                    }
                } else {
                    // If clicked outside any node, clear focus and deactivate IME
                    if let Some(old_focused_id) = self.runtime_state.interaction.focused {
                        if let Some(old_node) = ir.nodes.get(&old_focused_id) {
                            if let Op::Semantics(s) = &old_node.op {
                                if s.role == fission_ir::semantics::Role::TextInput {
                                    if let Some(ime_handler) = &self.ime_handler {
                                        ime_handler.set_ime_allowed(false);
                                    }
                                }
                            }
                        }
                    }
                    self.runtime_state.interaction.set_focused(None);
                }
            }
            InputEvent::Pointer(PointerEvent::Up { point, .. }) => {
                if let Some(hit_node_id) =
                    hit_test_with_scroll(ir, layout, &self.runtime_state.scroll, point)
                {
                    // Clear pressed state
                    self.runtime_state.interaction.pressed.clear();

                    // Dispatch at most one action on pointer up, closest semantics first
                    let mut current_id = Some(hit_node_id);
                    while let Some(node_id) = current_id {
                        if let Some(node) = ir.nodes.get(&node_id) {
                            if let Op::Semantics(semantics) = &node.op {
                                if semantics.role == fission_ir::semantics::Role::TextInput {
                                    // TextInput only takes focus on click, no action dispatch
                                } else if let Some(action_entry) = semantics.actions.entries.first() {
                                    if let Some(payload) = &action_entry.payload_data {
                                        let envelope = ActionEnvelope {
                                            id: ActionId::from_u128(action_entry.action_id),
                                            payload: payload.clone(),
                                        };
                                        diag::emit(
                                            diag::DiagCategory::Input,
                                            diag::DiagLevel::Debug,
                                            diag::DiagEventKind::InputEvent { kind: "pointer_up_dispatch".into(), target: Some(node_id.as_u128()), position: Some((point.x, point.y)) },
                                        );
                                        return self.dispatch(envelope, node_id);
                                    }
                                    // If no payload (dynamic action), ignore for click.
                                }
                            }
                            current_id = node.parent;
                        } else {
                            break;
                        }
                    }

                    // Clear drag start on pointer up
                    self.runtime_state.interaction.last_down_point = None;
                }
            }
            InputEvent::Pointer(PointerEvent::Up { point: _, .. }) => {
                self.runtime_state.interaction.pressed.clear();
                self.runtime_state.interaction.last_down_point = None;
            }
            _ => {}
        }
        Ok(())
    }
}