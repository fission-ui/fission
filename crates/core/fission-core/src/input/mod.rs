use crate::env::{
    Clipboard, ContextMenuState, InteractionStateMap, ScrollStateMap, SelectableTextStateMap,
    TextEditStateMap,
};
use crate::event::InputEvent;
use crate::{ActionEnvelope, ActionId, ActionInput, UpdateTextInput};
use fission_ir::semantics::ActionTrigger;
use fission_ir::{CoreIR, Op, Semantics, WidgetId};
use fission_layout::{LayoutSnapshot, TextMeasurer};
use std::sync::Arc;

pub mod canvas;
pub mod gesture;
pub mod hover;
pub mod selectable_text;
pub mod slider;
pub mod text;
pub mod viewport;

mod editing_convention;
pub use editing_convention::TextEditingConvention;

pub struct ControllerContext<'a> {
    pub ir: &'a CoreIR,
    pub layout: &'a LayoutSnapshot,
    pub text_edit: &'a mut TextEditStateMap,
    pub selectable_text: &'a mut SelectableTextStateMap,
    pub context_menu: &'a mut ContextMenuState,
    pub interaction: &'a mut InteractionStateMap,
    pub scroll: &'a mut ScrollStateMap,
    pub viewport: &'a viewport::ViewportStateMap,
    pub gesture: &'a mut crate::env::GestureState,
    pub editing_convention: TextEditingConvention,
    /// Monotonic runtime time used for deterministic multi-click and hold gestures.
    pub current_time: crate::time::CurrentTime,
    pub clipboard: Option<&'a Arc<dyn Clipboard>>,
    pub measurer: Option<&'a Arc<dyn TextMeasurer>>,
    // We queue actions here instead of dispatching immediately to keep Controller pure logic
    pub dispatched_actions: Vec<(WidgetId, ActionEnvelope, ActionInput)>,
}

pub trait InputController {
    fn handle_event(&mut self, ctx: &mut ControllerContext, event: &InputEvent) -> bool;
}

/// Builds the action envelope and event input for one committed text edit.
///
/// This is the shared integration boundary for the native text controller,
/// accessibility adapters, browser islands, and other interactive shells.
/// The bound action payload is preserved and the complete live edit is carried
/// separately in [`ActionInput::TextChanged`].
#[doc(hidden)]
pub fn prepare_text_input_change(
    semantics: &Semantics,
    node_id: WidgetId,
    new_text: String,
    new_caret: usize,
    new_anchor: usize,
) -> Option<(ActionEnvelope, ActionInput)> {
    let entry = semantics
        .actions
        .entries
        .iter()
        .find(|entry| entry.trigger == ActionTrigger::TextChanged)?;

    // Lowered text inputs always retain a payload. Preserve an empty payload
    // for malformed external IR so reducer dispatch reports a structured
    // deserialization failure instead of silently dropping the edit here.
    let payload = entry.payload_data.clone().unwrap_or_default();
    let selection = crate::TextSelection::new(
        &new_text,
        new_anchor,
        new_caret,
        crate::TextAffinity::Downstream,
    )
    .unwrap_or_else(|_| crate::TextSelection::collapsed(crate::TextPosition::at_end(&new_text)));
    let new_value = crate::TextEditingValue::new(new_text.clone(), selection, None)
        .expect("selection was validated against the same text");
    let old_value =
        crate::TextEditingValue::from_text(semantics.value.as_deref().unwrap_or_default());
    let input = ActionInput::TextChanged(UpdateTextInput {
        node_id,
        new_text: new_text.clone(),
        new_caret,
        new_anchor,
        old_value,
        new_value,
        source: crate::TextEditSource::Programmatic,
        phase: crate::TextEditPhase::Committed,
        editing_action: None,
        validation_state: None,
        validation_message: None,
    });

    Some((
        ActionEnvelope {
            id: ActionId::from_u128(entry.action_id),
            payload,
        },
        input,
    ))
}

/// Builds a payload-preserving action from one complete text transaction.
#[doc(hidden)]
pub fn prepare_text_input_edit(
    semantics: &Semantics,
    node_id: WidgetId,
    result: crate::TextEditResult,
) -> Option<(ActionEnvelope, ActionInput)> {
    let entry = semantics
        .actions
        .entries
        .iter()
        .find(|entry| entry.trigger == ActionTrigger::TextChanged)?;
    let payload = entry.payload_data.clone().unwrap_or_default();
    Some((
        ActionEnvelope {
            id: ActionId::from_u128(entry.action_id),
            payload,
        },
        ActionInput::TextChanged(UpdateTextInput::from_values(
            node_id,
            result.old_value,
            result.new_value,
            result.source,
            result.phase,
        )),
    ))
}

#[doc(hidden)]
pub fn prepare_scoped_text_input_edit(
    ir: &CoreIR,
    semantics: &Semantics,
    node_id: WidgetId,
    result: crate::TextEditResult,
) -> Option<(ActionEnvelope, ActionInput)> {
    let (envelope, input) = prepare_text_input_edit(semantics, node_id, result)?;
    Some((envelope, scoped_action_input(ir, node_id, input)))
}

/// Builds a payload-preserving action for a text-session lifecycle event.
///
/// Focus, blur, validation, submit, and completion do not replace the bound
/// action payload with the live field value. The current complete editing
/// value travels through the same typed input used by text mutations.
#[doc(hidden)]
pub fn prepare_scoped_text_session_action(
    ir: &CoreIR,
    semantics: &Semantics,
    node_id: WidgetId,
    trigger: ActionTrigger,
    value: crate::TextEditingValue,
    source: crate::TextEditSource,
    phase: crate::TextEditPhase,
) -> Option<(ActionEnvelope, ActionInput)> {
    let entry = semantics
        .actions
        .entries
        .iter()
        .find(|entry| entry.trigger == trigger)?;
    let envelope = ActionEnvelope {
        id: ActionId::from_u128(entry.action_id),
        payload: entry.payload_data.clone().unwrap_or_default(),
    };
    let input = ActionInput::TextChanged(UpdateTextInput::from_values(
        node_id,
        value.clone(),
        value,
        source,
        phase,
    ));
    Some((envelope, scoped_action_input(ir, node_id, input)))
}

/// Builds and action-scopes one committed text edit.
///
/// Interactive shells should use this boundary so native, accessibility, and
/// browser-originated edits all preserve the same action payload and resolve
/// the nearest ancestor action scope consistently.
#[doc(hidden)]
pub fn prepare_scoped_text_input_change(
    ir: &CoreIR,
    semantics: &Semantics,
    node_id: WidgetId,
    new_text: String,
    new_caret: usize,
    new_anchor: usize,
) -> Option<(ActionEnvelope, ActionInput)> {
    let (envelope, input) =
        prepare_text_input_change(semantics, node_id, new_text, new_caret, new_anchor)?;
    Some((envelope, scoped_action_input(ir, node_id, input)))
}

pub(crate) fn action_scope_for_node(ir: &CoreIR, node_id: WidgetId) -> Option<u128> {
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

pub(crate) fn scoped_action_input(
    ir: &CoreIR,
    target: WidgetId,
    input: ActionInput,
) -> ActionInput {
    if let Some(scope_id) = action_scope_for_node(ir, target) {
        ActionInput::scoped_raw(scope_id, target, input)
    } else {
        input
    }
}
