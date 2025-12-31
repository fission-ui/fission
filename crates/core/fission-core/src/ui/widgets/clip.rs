use crate::lowering::{LoweringContext, NodeBuilder};
use crate::ui::traits::Lower;
use crate::ui::Node;
use fission_ir::{LayoutOp, NodeId, Op};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Clip {
    pub id: Option<NodeId>,
    pub path: Option<String>,
    pub child: Box<Node>,
}

impl Default for Clip {
    fn default() -> Self {
        Self {
            id: None,
            path: None,
            child: Box::new(crate::ui::widgets::spacer::Spacer::default().into_node()),
        }
    }
}

impl Clip {
    pub fn new(child: Node) -> Self {
        Self {
            child: Box::new(child),
            ..Default::default()
        }
    }

    pub fn path(mut self, path: impl Into<String>) -> Self {
        self.path = Some(path.into());
        self
    }

    pub fn into_node(self) -> Node {
        Node::Clip(self)
    }
}

impl Lower for Clip {
    fn lower(&self, cx: &mut LoweringContext) -> NodeId {
        let id = self.id.unwrap_or_else(|| cx.next_node_id());
        
        cx.push_scope(id);
        let child_id = self.child.lower(cx);
        cx.pop_scope();
        
        let mut builder = NodeBuilder::new(id, Op::Layout(LayoutOp::Clip {
            path: self.path.clone(),
        }));
        
        builder.add_child(child_id);
        builder.build(cx)
    }
}
