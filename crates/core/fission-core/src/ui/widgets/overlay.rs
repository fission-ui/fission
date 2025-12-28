use crate::lowering::{LoweringContext, NodeBuilder};
use crate::ui::traits::Lower;
use crate::ui::{Node, Text, TextContent};
use fission_ir::{LayoutOp, NodeId, Op};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Overlay {
    pub id: Option<NodeId>,
    pub content: Box<Node>,
    pub overlay: Box<Node>,
}

impl Default for Overlay {
    fn default() -> Self {
        Self {
            id: None,
            content: Box::new(Node::Text(Text {
                content: TextContent::Literal("".into()),
                ..Default::default()
            })),
            overlay: Box::new(Node::Text(Text {
                content: TextContent::Literal("".into()),
                ..Default::default()
            })),
        }
    }
}

impl Lower for Overlay {
    fn lower(&self, cx: &mut LoweringContext) -> NodeId {
        let id = self.id.unwrap_or_else(|| cx.next_node_id());
        
        cx.push_scope(id);

        // Build overlay child wrapped in AbsoluteFill so it layers over content.
        let overlay_child_id = self.overlay.lower(cx);
        let mut overlay_fill =
            NodeBuilder::new(cx.next_node_id(), Op::Layout(LayoutOp::AbsoluteFill));
        overlay_fill.add_child(overlay_child_id);
        let overlay_fill_id = overlay_fill.build(cx);

        // Stack container: content first, overlay second.
        let mut stack = NodeBuilder::new(id, Op::Layout(LayoutOp::ZStack));
        stack.add_child(self.content.lower(cx));
        stack.add_child(overlay_fill_id);
        
        cx.pop_scope();
        
        stack.build(cx)
    }
}
