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
pub mod context; // New
pub mod diff;
pub mod effect; // New
pub mod env;
pub mod event;
pub mod hit_test;
pub mod input;
pub mod lowering;
pub mod media; 
pub mod registry;
pub mod runtime;
pub mod time;
pub mod ui;

pub mod view;

#[cfg(test)]
mod tests;

use crate::env::ActiveAnimation;
pub use action::{Action, ActionEnvelope, ActionId, AppState};
pub use context::{ReducerContext, Effects}; // New
pub use effect::{Effect, EffectEnvelope, EffectPayload, ActionInput, SystemEffect}; // New
pub use env::{Env, InteractionStateMap, RuntimeState, ScrollStateMap, Clipboard, ImeHandler};
pub use runtime::Runtime;

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
    dyn FnMut(
        &mut HashMap<TypeId, Box<dyn AppState>>, 
        &ActionEnvelope, 
        NodeId,
        &mut Vec<EffectEnvelope>,
        &ActionInput
    ) -> Result<()>
        + Send
        + Sync,
>;

