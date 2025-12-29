use crate::action::{Action, ActionEnvelope, ActionId, AppState, Reducer, Animate};
use crate::env::{Env, RuntimeState, VideoStatus, VideoStateMap, AnimationStateMap, ActiveAnimation, Clock, InteractionStateMap, ScrollStateMap}; 
use fission_ir::{CoreIR, NodeId, Op, LayoutOp};
use fission_layout::{LayoutSnapshot, LayoutPoint};
use fission_semantics::{InputEvent, PointerEvent, PointerButton, KeyCode, KeyEvent};
use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::sync::Arc;
use anyhow::Result;
use serde_json;

pub struct Runtime {
    pub app_states: HashMap<TypeId, Box<dyn Any>>,
    pub runtime_state: RuntimeState,
    pub reducers: HashMap<ActionId, Box<dyn Fn(&mut dyn Any, &ActionEnvelope, NodeId) -> Result<()>>>,
    pub clock: Clock,
}

impl Default for Runtime {
    fn default() -> Self {
        Self {
            app_states: HashMap::new(),
            runtime_state: RuntimeState::default(),
            reducers: HashMap::new(),
            clock: Clock::default(),
        }
    }
}

impl Runtime {
    pub fn add_app_state<S: AppState + 'static>(&mut self, state: Box<S>) -> Result<()> {
        self.app_states.insert(TypeId::of::<S>(), state);
        Ok(())
    }

    pub fn get_app_state<S: AppState + 'static>(&self) -> Option<&S> {
        self.app_states.get(&TypeId::of::<S>()).and_then(|b| b.downcast_ref::<S>())
    }
    
    pub fn get_app_state_mut<S: AppState + 'static>(&mut self) -> Option<&mut S> {
        self.app_states.get_mut(&TypeId::of::<S>()).and_then(|b| b.downcast_mut::<S>())
    }

    pub fn register_reducer<S: AppState + 'static>(
        &mut self,
        action_id: ActionId,
        reducer: fn(&mut S, &ActionEnvelope, NodeId) -> Result<()>,
    ) -> Result<()> {
        let boxed_reducer = Box::new(move |state_any: &mut dyn Any, env: &ActionEnvelope, target: NodeId| {
            if let Some(state) = state_any.downcast_mut::<S>() {
                reducer(state, env, target)
            } else {
                Err(anyhow::anyhow!("State type mismatch for reducer"))
            }
        });
        self.reducers.insert(action_id, boxed_reducer);
        Ok(())
    }
    
    pub fn dispatch(&mut self, envelope: ActionEnvelope, target: NodeId) -> Result<()> {
        if envelope.id == Animate::static_id() {
            if let Ok(anim) = serde_json::from_slice::<Animate>(&envelope.payload) {
                self.runtime_state.animation.active.push(ActiveAnimation {
                    key: anim.property.clone(),
                    node_id: anim.target,
                    property: anim.property.clone(),
                    start_value: anim.start,
                    end_value: anim.end,
                    start_time: self.clock.now(),
                    duration: anim.duration,
                });
                return Ok(());
            }
        }

        if let Some(reducer) = self.reducers.get(&envelope.id) {
            for state in self.app_states.values_mut() {
                if let Ok(_) = reducer(state.as_mut(), &envelope, target) {
                    return Ok(());
                }
            }
        }
        Ok(())
    }
    
    pub fn clear_reducers(&mut self) {
        self.reducers.clear();
    }
    
    pub fn absorb_registry(&mut self, registry: HashMap<ActionId, Box<dyn Fn(&mut dyn Any, &ActionEnvelope, NodeId) -> Result<()>>>) {
        self.reducers.extend(registry);
    }

    pub fn tick(&mut self, dt_ms: u64) -> Result<()> {
        self.clock.advance(dt_ms);
        
        let mut finished_anims = Vec::new();
        for (i, anim) in self.runtime_state.animation.active.iter_mut().enumerate() {
            let elapsed = self.clock.now().saturating_sub(anim.start_time);
            let progress = (elapsed as f32 / anim.duration as f32).min(1.0);
            
            let val = anim.start_value + (anim.end_value - anim.start_value) * progress;
            self.runtime_state.animation.values.insert((anim.node_id, anim.property.clone()), val);
            
            if elapsed >= anim.duration {
                finished_anims.push(i);
            }
        }
        
        for i in finished_anims.into_iter().rev() {
            self.runtime_state.animation.active.remove(i);
        }
        Ok(())
    }
    
    pub fn clock(&self) -> &Clock {
        &self.clock
    }

    pub fn handle_input(&mut self, event: InputEvent, ir: &CoreIR, snapshot: &LayoutSnapshot) -> Result<()> {
        match event {
            InputEvent::Pointer(pe) => self.handle_pointer(pe, ir, snapshot),
            InputEvent::Keyboard(ke) => self.handle_keyboard(ke, ir, snapshot),
        }
    }

    fn handle_pointer(&mut self, event: PointerEvent, ir: &CoreIR, snapshot: &LayoutSnapshot) -> Result<()> {
        let point = match event {
            PointerEvent::Move { point } | PointerEvent::Down { point, .. } | PointerEvent::Up { point, .. } => point,
            PointerEvent::Scroll { point, .. } => point,
        };

        let target_id = self.hit_test(point, ir, snapshot);
        
        match event {
            PointerEvent::Move { .. } => {
                if let Some(id) = target_id {
                    self.runtime_state.interaction.hovered.clear();
                    self.runtime_state.interaction.set_hovered(id, true);
                } else {
                    self.runtime_state.interaction.hovered.clear();
                }
            },
            PointerEvent::Down { .. } => {
                if let Some(id) = target_id {
                    self.runtime_state.interaction.set_pressed(id, true);
                    self.runtime_state.interaction.set_focused(Some(id));
                }
            },
            PointerEvent::Up { .. } => {
                self.runtime_state.interaction.pressed.clear();
            },
            PointerEvent::Scroll { delta, .. } => {
                if let Some(id) = target_id {
                    let mut current = Some(id);
                    while let Some(curr_id) = current {
                        if let Some(node) = ir.nodes.get(&curr_id) {
                            if let Op::Layout(LayoutOp::Scroll { .. }) = &node.op {
                                if let Some(geom) = snapshot.nodes.get(&curr_id) {
                                    let current_offset = self.runtime_state.scroll.get_offset(curr_id);
                                    let max_offset = (geom.content_size.height - geom.rect.height()).max(0.0);
                                    let new_offset = (current_offset + delta.y).clamp(0.0, max_offset);
                                    
                                    self.runtime_state.scroll.set_offset(curr_id, new_offset);
                                }
                                break;
                            }
                            current = node.parent;
                        } else {
                            break;
                        }
                    }
                }
            },
        }
        Ok(())
    }

    fn handle_keyboard(&mut self, _event: KeyEvent, _ir: &CoreIR, _snapshot: &LayoutSnapshot) -> Result<()> {
        Ok(())
    }

    pub fn hit_test(&self, point: LayoutPoint, ir: &CoreIR, snapshot: &LayoutSnapshot) -> Option<NodeId> {
        if let Some(root) = ir.root {
            return self.hit_test_recursive(root, point, ir, snapshot);
        }
        None
    }

    fn hit_test_recursive(&self, node_id: NodeId, point: LayoutPoint, ir: &CoreIR, snapshot: &LayoutSnapshot) -> Option<NodeId> {
        if let Some(geom) = snapshot.nodes.get(&node_id) {
            if geom.rect.contains(point) {
                if let Some(node) = ir.nodes.get(&node_id) {
                    for child in node.children.iter().rev() {
                        let child_point = point;
                        
                        if let Op::Layout(LayoutOp::Scroll { .. }) = &node.op {
                            if !geom.rect.contains(point) { continue; }
                        }
                        
                        if let Some(hit) = self.hit_test_recursive(*child, child_point, ir, snapshot) {
                            return Some(hit);
                        }
                    }
                }
                return Some(node_id);
            }
        }
        None
    }
}
