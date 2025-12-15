use serde::{Deserialize, Serialize};
use crate::lowering::LoweringContext;
use fission_ir::{
    op::{LayoutOp, Op, FlexDirection},
    NodeId
};
use crate::ui::{Node, traits::Lower};

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Scroll {
    pub id: Option<NodeId>,
    pub child: Option<Box<Node>>,
    pub direction: FlexDirection,
    pub width: Option<f32>,
    pub height: Option<f32>,
    pub show_scrollbar: bool,
}

impl Lower for Scroll {
    fn lower(&self, cx: &mut LoweringContext) -> NodeId {
        let mut child_ids = Vec::new();
        if let Some(child) = &self.child {
            child_ids.push(child.lower(cx));
        }

        let layout_id = self.id.unwrap_or_else(|| cx.next_node_id());
        
        cx.add_node(
            layout_id,
            Op::Layout(LayoutOp::Scroll { 
                direction: self.direction, 
                show_scrollbar: self.show_scrollbar,
                width: self.width,
                height: self.height,
                padding: [0.0; 4],
            }),
            child_ids.clone(),
        );
        
        for child_id in &child_ids {
            if let Some(node) = cx.ir.nodes.get_mut(child_id) {
                node.parent = Some(layout_id);
            }
        }

        layout_id
    }
}