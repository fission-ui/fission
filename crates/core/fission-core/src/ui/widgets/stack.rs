use crate::lowering::{LoweringContext, NodeBuilder};
use crate::ui::traits::Lower;
use crate::ui::Node;
use fission_ir::{LayoutOp, NodeId, Op};
use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Stack {
    pub id: Option<NodeId>,
    pub children: Vec<Node>,
}

impl Lower for Stack {
    fn lower(&self, cx: &mut LoweringContext) -> NodeId {
        let id = self.id.unwrap_or_else(|| cx.next_node_id());
        
        cx.push_scope(id);
        
        let mut builder = NodeBuilder::new(id, Op::Layout(LayoutOp::Stack));
        for child in &self.children {
            builder.add_child(child.lower(cx));
        }
        
        cx.pop_scope();
        
        builder.build(cx)
    }
}

