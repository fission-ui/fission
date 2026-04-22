use crate::lowering::{LoweringContext, NodeBuilder};
use crate::ui::traits::Lower;
use crate::ui::Node;
use crate::ActionEnvelope;
use fission_ir::{
    ActionEntry, NodeId, Op, Semantics, 
    semantics::{ActionTrigger, Role},
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GestureDetector {
    pub id: Option<NodeId>,
    pub child: Box<Node>,
    pub on_tap: Option<ActionEnvelope>,
    pub on_double_tap: Option<ActionEnvelope>,
    pub on_long_press: Option<ActionEnvelope>,
    pub on_drag_start: Option<ActionEnvelope>,
    pub on_drag_update: Option<ActionEnvelope>,
    pub on_drag_end: Option<ActionEnvelope>,
    pub on_hover_enter: Option<ActionEnvelope>,
    pub on_hover_exit: Option<ActionEnvelope>,
    pub on_drop: Option<ActionEnvelope>,
    pub on_secondary_click: Option<ActionEnvelope>,
    pub on_drag_enter: Option<ActionEnvelope>, // Drag over
    pub on_drag_leave: Option<ActionEnvelope>,
    pub drag_payload: Option<Vec<u8>>,
}

impl Default for GestureDetector {
    fn default() -> Self {
        Self {
            id: None,
            child: Box::new(crate::ui::widgets::spacer::Spacer::default().into_node()),
            on_tap: None,
            on_double_tap: None,
            on_long_press: None,
            on_secondary_click: None,
            on_drag_start: None,
            on_drag_update: None,
            on_drag_end: None,
            on_hover_enter: None,
            on_hover_exit: None,
            on_drop: None,
            on_drag_enter: None,
            on_drag_leave: None,
            drag_payload: None,
        }
    }
}

impl GestureDetector {
    pub fn new(child: Node) -> Self {
        Self {
            child: Box::new(child),
            ..Default::default()
        }
    }

    pub fn into_node(self) -> crate::ui::Node {
        crate::ui::Node::GestureDetector(self)
    }
}

impl Lower for GestureDetector {
    fn lower(&self, cx: &mut LoweringContext) -> NodeId {
        let id = self.id.unwrap_or_else(|| cx.next_node_id());
        
        // Lower child
        let child_id = self.child.lower(cx);
        
        // Build Semantics
        let mut semantics = Semantics {
            role: Role::Generic,
            label: None,
            value: None,
            actions: Default::default(),
            focusable: self.on_tap.is_some(), 
            multiline: false,
            masked: false,
            input_mask: None,
            ime_preedit_range: None,
            checked: None,
            disabled: false,
            draggable: self.on_drag_start.is_some() || self.on_drag_update.is_some() || self.drag_payload.is_some(),
            scrollable_x: false,
            scrollable_y: false,
            min_value: None,
            max_value: None,
            current_value: None,
            is_focus_scope: false,
            is_focus_barrier: false,
            drag_payload: self.drag_payload.clone(),
            hero_tag: None,
            focus_index: None, capture_tab: false, auto_indent: false,
        };

        if let Some(a) = &self.on_tap {
            semantics.actions.entries.push(ActionEntry {
                trigger: ActionTrigger::Default,
                action_id: a.id.as_u128(),
                payload_data: Some(a.payload.clone()),
            });
        }

        if let Some(a) = &self.on_secondary_click {
            semantics.actions.entries.push(ActionEntry {
                trigger: ActionTrigger::SecondaryClick,
                action_id: a.id.as_u128(),
                payload_data: Some(a.payload.clone()),
            });
        }

        if let Some(a) = &self.on_drag_start {
            semantics.actions.entries.push(ActionEntry {
                trigger: ActionTrigger::DragStart,
                action_id: a.id.as_u128(),
                payload_data: Some(a.payload.clone()),
            });
        }
        
        if let Some(a) = &self.on_drag_update {
            semantics.actions.entries.push(ActionEntry {
                trigger: ActionTrigger::DragUpdate,
                action_id: a.id.as_u128(),
                payload_data: Some(a.payload.clone()),
            });
        }
        
        if let Some(a) = &self.on_drag_end {
            semantics.actions.entries.push(ActionEntry {
                trigger: ActionTrigger::DragEnd,
                action_id: a.id.as_u128(),
                payload_data: Some(a.payload.clone()),
            });
        }
        
        if let Some(a) = &self.on_hover_enter {
            semantics.actions.entries.push(ActionEntry {
                trigger: ActionTrigger::HoverEnter,
                action_id: a.id.as_u128(),
                payload_data: Some(a.payload.clone()),
            });
        }
        
        if let Some(a) = &self.on_hover_exit {
            semantics.actions.entries.push(ActionEntry {
                trigger: ActionTrigger::HoverExit,
                action_id: a.id.as_u128(),
                payload_data: Some(a.payload.clone()),
            });
        }
        
        if let Some(a) = &self.on_drop {
            semantics.actions.entries.push(ActionEntry {
                trigger: ActionTrigger::Drop,
                action_id: a.id.as_u128(),
                payload_data: Some(a.payload.clone()),
            });
        }
        
        if let Some(a) = &self.on_drag_enter {
            semantics.actions.entries.push(ActionEntry {
                trigger: ActionTrigger::DragEnter,
                action_id: a.id.as_u128(),
                payload_data: Some(a.payload.clone()),
            });
        }
        
        if let Some(a) = &self.on_drag_leave {
            semantics.actions.entries.push(ActionEntry {
                trigger: ActionTrigger::DragLeave,
                action_id: a.id.as_u128(),
                payload_data: Some(a.payload.clone()),
            });
        }

        let mut node = NodeBuilder::new(id, Op::Semantics(semantics));
        node.add_child(child_id);
        node.build(cx)
    }
}
