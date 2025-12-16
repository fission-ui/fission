use crate::lowering::{LoweringContext, NodeBuilder};
use crate::ui::{traits::Lower, Node};
use fission_ir::{
    op::{FlexDirection, LayoutOp, Op},
    NodeId,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Scroll {
    pub id: Option<NodeId>,
    pub child: Option<Box<Node>>,
    pub direction: FlexDirection,
    pub width: Option<f32>,
    pub height: Option<f32>,
    pub show_scrollbar: bool,
}

impl Default for Scroll {
    fn default() -> Self {
        Self {
            id: None,
            child: None,
            direction: FlexDirection::Column,
            width: None,
            height: None,
            show_scrollbar: false,
        }
    }
}

impl Lower for Scroll {
    fn lower(&self, cx: &mut LoweringContext) -> NodeId {
        let layout_id = self.id.unwrap_or_else(|| cx.next_node_id());
        let mut builder = NodeBuilder::new(
            layout_id,
            Op::Layout(LayoutOp::Scroll {
                direction: self.direction,
                show_scrollbar: self.show_scrollbar,
                width: self.width,
                height: self.height,
                padding: [0.0; 4],
            }),
        );
        if let Some(child) = &self.child {
            builder.add_child(child.lower(cx));
        }
        builder.build(cx)
    }
}
