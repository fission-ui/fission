use crate::lowering::{wrap_zstack_child, LoweringContext, NodeBuilder};
use crate::ui::traits::Lower;
use crate::ui::Node;
use fission_ir::{semantics::Role, NodeId, Op, Semantics};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FocusScope {
    pub id: Option<NodeId>,
    pub children: Vec<Node>,
    pub is_barrier: bool,
}

impl Default for FocusScope {
    fn default() -> Self {
        Self {
            id: None,
            children: Vec::new(),
            is_barrier: false,
        }
    }
}

impl FocusScope {
    pub fn into_node(self) -> Node {
        Node::FocusScope(self)
    }
}

impl Lower for FocusScope {
    fn lower(&self, cx: &mut LoweringContext) -> NodeId {
        let id = self.id.unwrap_or_else(|| cx.next_node_id());

        cx.push_scope(id);
        let mut child_ids = Vec::new();
        for child in &self.children {
            child_ids.push(child.lower(cx));
        }
        cx.pop_scope();

        // Wrap children in a ZStack layout node
        let layout_id = cx.next_node_id();
        cx.push_scope(layout_id);
        let mut wrapped_children = Vec::with_capacity(child_ids.len());
        for cid in child_ids {
            wrapped_children.push(wrap_zstack_child(cx, cid));
        }
        cx.pop_scope();

        let mut layout_builder =
            NodeBuilder::new(layout_id, Op::Layout(fission_ir::LayoutOp::ZStack));
        for cid in wrapped_children {
            layout_builder.add_child(cid);
        }
        let layout_id = layout_builder.build(cx);

        let semantics = Semantics {
            role: Role::Generic,
            label: None,
            identifier: None,
            value: None,
            actions: Default::default(),
            focusable: false,
            multiline: false,
            masked: false,
            input_mask: None,
            ime_preedit_range: None,
            checked: None,
            disabled: false,
            read_only: false,
            autofocus: false,
            draggable: false,
            scrollable_x: false,
            scrollable_y: false,
            min_value: None,
            max_value: None,
            current_value: None,
            is_focus_scope: true,
            is_focus_barrier: self.is_barrier,
            drag_payload: None,
            hero_tag: None,
            focus_index: None,
            text_input_type: fission_ir::semantics::TextInputType::Text,
            text_input_action: fission_ir::semantics::TextInputAction::Done,
            text_capitalization: fission_ir::semantics::TextCapitalization::None,
            max_length: None,
            max_length_enforcement: fission_ir::semantics::MaxLengthEnforcement::Enforced,
            input_formatters: Vec::new(),
            autocorrect: true,
            enable_suggestions: true,
            spell_check: true,
            smart_dashes: true,
            smart_quotes: true,
            autofill_hints: Vec::new(),
            scroll_padding: None,
            capture_tab: false,
            auto_indent: false,
        };

        let mut node = NodeBuilder::new(id, Op::Semantics(semantics));
        node.add_child(layout_id);
        node.build(cx)
    }
}
