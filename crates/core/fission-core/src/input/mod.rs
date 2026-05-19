use crate::env::{Clipboard, InteractionStateMap, ScrollStateMap, TextEditStateMap};
use crate::event::InputEvent;
use crate::{ActionEnvelope, ActionInput};
use fission_ir::{CoreIR, NodeId, Op};
use fission_layout::{LayoutSnapshot, TextMeasurer};
use std::sync::Arc;

pub mod gesture;
pub mod hover;
pub mod slider;
pub mod text;

pub struct ControllerContext<'a> {
    pub ir: &'a CoreIR,
    pub layout: &'a LayoutSnapshot,
    pub text_edit: &'a mut TextEditStateMap,
    pub interaction: &'a mut InteractionStateMap,
    pub scroll: &'a mut ScrollStateMap,
    pub gesture: &'a mut crate::env::GestureState,
    pub clipboard: Option<&'a Arc<dyn Clipboard>>,
    pub measurer: Option<&'a Arc<dyn TextMeasurer>>,
    // We queue actions here instead of dispatching immediately to keep Controller pure logic
    pub dispatched_actions: Vec<(NodeId, ActionEnvelope, ActionInput)>,
}

pub trait InputController {
    fn handle_event(&mut self, ctx: &mut ControllerContext, event: &InputEvent) -> bool;
}

pub(crate) fn action_scope_for_node(ir: &CoreIR, node_id: NodeId) -> Option<u128> {
    let mut current_id = Some(node_id);
    while let Some(id) = current_id {
        let Some(node) = ir.nodes.get(&id) else {
            break;
        };
        if let Op::Semantics(semantics) = &node.op {
            if let Some(scope_id) = semantics.action_scope_id {
                return Some(scope_id);
            }
        }
        current_id = node.parent;
    }
    None
}

pub(crate) fn scoped_action_input(ir: &CoreIR, target: NodeId, input: ActionInput) -> ActionInput {
    if let Some(scope_id) = action_scope_for_node(ir, target) {
        ActionInput::scoped_raw(scope_id, target, input)
    } else {
        input
    }
}
