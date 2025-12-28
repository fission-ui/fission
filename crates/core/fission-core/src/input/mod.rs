use crate::env::{Clipboard, InteractionStateMap, ScrollStateMap, TextEditStateMap};
use crate::event::InputEvent;
use crate::ActionEnvelope;
use fission_ir::{CoreIR, NodeId};
use fission_layout::{LayoutSnapshot, TextMeasurer};
use std::sync::Arc;

pub mod text;
pub mod slider;

pub struct ControllerContext<'a> {
    pub ir: &'a CoreIR,
    pub layout: &'a LayoutSnapshot,
    pub text_edit: &'a mut TextEditStateMap,
    pub interaction: &'a mut InteractionStateMap,
    pub scroll: &'a mut ScrollStateMap,
    pub ime_preedit: &'a mut Option<(NodeId, String)>,
    pub clipboard: Option<&'a Arc<dyn Clipboard>>,
    pub measurer: Option<&'a Arc<dyn TextMeasurer>>,
    // We queue actions here instead of dispatching immediately to keep Controller pure logic
    pub dispatched_actions: Vec<(NodeId, ActionEnvelope)>,
}

pub trait InputController {
    fn handle_event(&mut self, ctx: &mut ControllerContext, event: &InputEvent) -> bool;
}