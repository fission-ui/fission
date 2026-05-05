use crate::lowering::{LoweringContext, NodeBuilder};
use crate::ui::traits::Lower;
use crate::ui::Node;
use fission_ir::{LayoutOp, NodeId, Op};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SafeArea {
    pub id: Option<NodeId>,
    pub child: Box<Node>,
}

impl Default for SafeArea {
    fn default() -> Self {
        Self {
            id: None,
            child: Box::new(crate::ui::widgets::spacer::Spacer::default().into_node()),
        }
    }
}

impl SafeArea {
    pub fn into_node(self) -> Node {
        Node::SafeArea(self)
    }
}

impl Lower for SafeArea {
    fn lower(&self, cx: &mut LoweringContext) -> NodeId {
        let id = self.id.unwrap_or_else(|| cx.next_node_id());
        let insets = &cx.env.window_insets;

        cx.push_scope(id);
        let child_id = self.child.lower(cx);
        cx.pop_scope();

        // SafeArea is just a Box with padding derived from window_insets
        let mut builder = NodeBuilder::new(
            id,
            Op::Layout(LayoutOp::Box {
                width: None,
                height: None,
                min_width: None,
                max_width: None,
                min_height: None,
                max_height: None,
                padding: [insets.left, insets.right, insets.top, insets.bottom],
                flex_grow: 1.0,
                flex_shrink: 1.0,
                aspect_ratio: None,
            }),
        );

        builder.add_child(child_id);
        builder.build(cx)
    }
}
