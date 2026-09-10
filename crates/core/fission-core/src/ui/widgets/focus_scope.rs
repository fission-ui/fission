use crate::internal::InternalLower;
use crate::lowering::{wrap_zstack_child, InternalIrBuilder, InternalLoweringCx};
use crate::ui::Widget;
use fission_ir::{Op, Semantics, WidgetId};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FocusScope {
    pub id: Option<WidgetId>,
    pub children: Vec<Widget>,
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

impl FocusScope {}

impl InternalLower for FocusScope {
    fn lower(&self, cx: &mut InternalLoweringCx) -> WidgetId {
        let id = self.id.map(Into::into).unwrap_or_else(|| cx.next_node_id());

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
            InternalIrBuilder::new(layout_id, Op::Layout(fission_ir::LayoutOp::ZStack));
        for cid in wrapped_children {
            layout_builder.add_child(cid);
        }
        let layout_id = layout_builder.build(cx);

        let semantics = Semantics {
            is_focus_scope: true,
            is_focus_barrier: self.is_barrier,
            ..Semantics::default()
        };

        let mut node = InternalIrBuilder::new(id, Op::Semantics(semantics));
        node.add_child(layout_id);
        node.build(cx)
    }
}
