use crate::lowering::{LoweringContext, NodeBuilder};
use crate::ui::traits::Lower;
use crate::ui::Node;
use fission_ir::{op::{LayoutOp, Op}, NodeId};
use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Spacer {
    pub id: Option<NodeId>,
    pub width: Option<f32>,
    pub height: Option<f32>,
    pub flex_grow: f32,
}

impl Spacer {
    pub fn into_node(self) -> Node {
        Node::Spacer(self)
    }
}

impl Lower for Spacer {
    fn lower(&self, cx: &mut LoweringContext) -> NodeId {
        let id = self.id.unwrap_or_else(|| cx.next_node_id());
        
        NodeBuilder::new(
            id,
            Op::Layout(LayoutOp::Box {
                width: self.width,
                height: self.height,
                min_width: None,
                max_width: None,
                min_height: None,
                max_height: None,
                padding: [0.0; 4],
                flex_grow: self.flex_grow,
                flex_shrink: 0.0,
                aspect_ratio: None,
            }),
        )
        .build(cx)
    }
}


