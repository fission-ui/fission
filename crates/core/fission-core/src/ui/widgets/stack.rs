use crate::lowering::{LoweringContext, NodeBuilder, wrap_zstack_child};
use crate::ui::traits::Lower;
use crate::ui::Node;
use fission_ir::{LayoutOp, NodeId, Op};
use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct ZStack {
    pub id: Option<NodeId>,
    pub children: Vec<Node>,
}

impl ZStack {
    pub fn children(mut self, children: Vec<Node>) -> Self {
        self.children = children;
        self
    }

    pub fn into_node(self) -> Node {
        Node::ZStack(self)
    }
}

impl Lower for ZStack {
    fn lower(&self, cx: &mut LoweringContext) -> NodeId {
        let id = self.id.unwrap_or_else(|| cx.next_node_id());
        
        cx.push_scope(id);
        
        let mut builder = NodeBuilder::new(id, Op::Layout(LayoutOp::ZStack));
        for child in &self.children {
            let child_id = child.lower(cx);
            builder.add_child(wrap_zstack_child(cx, child_id));
        }
        
        cx.pop_scope();
        
        builder.build(cx)
    }
}
