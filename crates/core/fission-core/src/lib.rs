use anyhow::{anyhow, Result};
use fission_ir::CoreIR; // Removed NodeId, Op
use fission_layout::{LayoutSnapshot, LayoutPoint};
use std::any::{Any, TypeId};
use std::collections::HashMap;
use lazy_static::lazy_static;
use downcast_rs::Downcast;

pub mod action;
pub mod time;
pub mod lowering;
pub mod event;
pub mod hit_test;
pub mod registry;
pub mod env;
pub mod ui;
pub mod view;

pub use action::{Action, ActionId, AppState, ActionEnvelope};
pub use time::{Clock, CurrentTime};
pub use lowering::{LoweringContext};
pub use event::{InputEvent, PointerEvent, PointerButton, KeyEvent, KeyCode, LifecycleEvent};
pub use fission_ir::op; 
pub use fission_ir::{Op, NodeId}; // Export Op and NodeId
pub use registry::{BuildCtx, ActionRegistry, Handler};
pub use env::{Env, RuntimeState, InteractionStateMap};
pub use ui::{Node, Row, Column, Text, Button, CustomNode, Lower, LowerDyn};
pub use view::{View, Selector, Widget};
use hit_test::hit_test;

// ... Rest unchanged
// Concrete Action implementations for clock control
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

pub type BoxedReducer = Box<dyn FnMut(&mut HashMap<TypeId, Box<dyn AppState>>, &ActionEnvelope, NodeId) -> Result<()> + Send + Sync>;

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
        
        runtime.add_app_state(Box::new(Clock::default())).expect("Failed to add Clock state");

        runtime.register_reducer::<Clock>(*TICK_ACTION_ID, |state: &mut Clock, action: &ActionEnvelope, _target| {
            let tick_action: Tick = serde_json::from_slice(&action.payload).map_err(|e| anyhow!("Failed to deserialize Tick: {}", e))?;
            state.advance_by(tick_action.dt)
        }).expect("Failed to register Tick reducer");

        runtime.register_reducer::<Clock>(*ADVANCE_TO_ACTION_ID, |state: &mut Clock, action: &ActionEnvelope, _target| {
            let advance_action: AdvanceTo = serde_json::from_slice(&action.payload).map_err(|e| anyhow!("Failed to deserialize AdvanceTo: {}", e))?;
            state.set_to(advance_action.time)
        }).expect("Failed to register AdvanceTo reducer");
        
        runtime
    }
}

impl Runtime {
    pub fn clear_reducers(&mut self) {
        self.reducers.clear();
        
        self.register_reducer::<Clock>(*TICK_ACTION_ID, |state: &mut Clock, action: &ActionEnvelope, _target| {
            let tick_action: Tick = serde_json::from_slice(&action.payload).map_err(|e| anyhow!("Failed to deserialize Tick: {}", e))?;
            state.advance_by(tick_action.dt)
        }).expect("Failed to register Tick reducer");

        self.register_reducer::<Clock>(*ADVANCE_TO_ACTION_ID, |state: &mut Clock, action: &ActionEnvelope, _target| {
            let advance_action: AdvanceTo = serde_json::from_slice(&action.payload).map_err(|e| anyhow!("Failed to deserialize AdvanceTo: {}", e))?;
            state.set_to(advance_action.time)
        }).expect("Failed to register AdvanceTo reducer");
    }

    pub fn absorb_registry<S: AppState>(&mut self, registry: ActionRegistry<S>) {
        let new_reducers = registry.into_runtime_reducers();
        for (id, mut list) in new_reducers {
            self.reducers.entry(id).or_default().append(&mut list);
        }
    }

    pub fn clock(&self) -> &Clock {
        self.get_app_state::<Clock>().expect("Clock state must always be present")
    }

    pub fn get_app_state<S: AppState + 'static>(&self) -> Option<&S> {
        self.app_states.get(&TypeId::of::<S>()).and_then(|s_box| s_box.downcast_ref::<S>())
    }

    pub fn get_app_state_mut<S: AppState + 'static>(&mut self) -> Option<&mut S> {
        self.app_states.get_mut(&TypeId::of::<S>()).and_then(|s_box| s_box.downcast_mut::<S>())
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
            move |app_states: &mut HashMap<TypeId, Box<dyn AppState>>, action: &ActionEnvelope, target: NodeId| -> Result<()> {
                if let Some(state_box) = app_states.get_mut(&state_type_id) {
                    let concrete_state = state_box.downcast_mut::<S>()
                        .ok_or_else(|| anyhow!("Failed to downcast AppState to concrete type for reducer"))?;
                    reducer_fn(concrete_state, action, target)
                } else {
                    anyhow::bail!("Target AppState for reducer not found in runtime.");
                }
            },
        );

        self.reducers.entry(action_id).or_default().push(boxed_reducer);
        Ok(())
    }

    pub fn dispatch(&mut self, action: ActionEnvelope, target: NodeId) -> Result<()> {
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

    pub fn handle_input(
        &mut self,
        event: InputEvent,
        ir: &CoreIR,
        layout: &LayoutSnapshot,
    ) -> Result<()> {
        match event {
            InputEvent::Pointer(PointerEvent::Move { point, .. }) => { // Added Move handling
                let hit = hit_test(ir, layout, point);
                
                // 1. Calculate new hover set
                let mut new_hovered = std::collections::HashSet::new();
                if let Some(mut node_id) = hit {
                    // Walk up from hit node to root
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
                
                // 2. Update interaction state (Simple clear and set for v1)
                // In a real app we would diff to avoid redraws if unchanged.
                // For now, we update the map.
                
                // We need to know which nodes were previously hovered to clear them.
                // Or we can just iterate the map? 
                // RuntimeState::interaction::hovered is HashMap<NodeId, bool>.
                
                // Clear old hovered
                self.runtime_state.interaction.hovered.clear();
                
                // Set new hovered
                for id in new_hovered {
                    self.runtime_state.interaction.set_hovered(id, true);
                }
                
                // TODO: Return true if changed?
            }
            InputEvent::Pointer(PointerEvent::Down { point, .. }) => {
                if let Some(hit_node_id) = hit_test(ir, layout, point) {
                    // Update Interaction State
                    self.runtime_state.interaction.set_pressed(hit_node_id, true);
                    // (Should also clear previous pressed?)
                    
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
                }
            }
            InputEvent::Pointer(PointerEvent::Up { point: _, .. }) => { // Ignored point for now
                // Clear all pressed state on Up
                self.runtime_state.interaction.pressed.clear();
            }
            _ => {}
        }
        Ok(())
    }
}