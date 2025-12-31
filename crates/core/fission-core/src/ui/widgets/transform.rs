use crate::lowering::{LoweringContext, NodeBuilder};
use crate::ui::traits::Lower;
use crate::ui::Node;
use fission_ir::{LayoutOp, NodeId, Op};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transform {
    pub id: Option<NodeId>,
    pub transform: [f32; 16],
    pub child: Box<Node>,
}

impl Default for Transform {
    fn default() -> Self {
        Self {
            id: None,
            transform: [
                1.0, 0.0, 0.0, 0.0,
                0.0, 1.0, 0.0, 0.0,
                0.0, 0.0, 1.0, 0.0,
                0.0, 0.0, 0.0, 1.0,
            ],
            child: Box::new(crate::ui::widgets::spacer::Spacer::default().into_node()),
        }
    }
}

impl Transform {
    pub fn new(child: Node, transform: [f32; 16]) -> Self {
        Self {
            child: Box::new(child),
            transform,
            ..Default::default()
        }
    }

    pub fn into_node(self) -> Node {
        Node::Transform(self)
    }
}

impl Lower for Transform {
    fn lower(&self, cx: &mut LoweringContext) -> NodeId {
        let id = self.id.unwrap_or_else(|| cx.next_node_id());
        
        cx.push_scope(id);
        let child_id = self.child.lower(cx);
        cx.pop_scope();
        
        let mut builder = NodeBuilder::new(id, Op::Layout(LayoutOp::Transform {
            transform: self.transform,
        }));
        
        builder.add_child(child_id);
        builder.build(cx)
    }
}
