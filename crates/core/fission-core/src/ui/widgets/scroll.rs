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
    pub flex_grow: f32,
    pub flex_shrink: f32,
}

impl Scroll {
    pub fn into_node(self) -> Node {
        Node::Scroll(self)
    }
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
            flex_grow: 0.0,
            flex_shrink: 0.0,
        }
    }
}

impl Lower for Scroll {
    fn lower(&self, cx: &mut LoweringContext) -> NodeId {
        let layout_id = self.id.unwrap_or_else(|| cx.next_node_id());
        
        cx.push_scope(layout_id);

        let mut builder = NodeBuilder::new(
            layout_id,
            Op::Layout(LayoutOp::Scroll {
                direction: self.direction,
                show_scrollbar: self.show_scrollbar,
                width: self.width,
                height: self.height,
                min_width: None,
                max_width: None,
                min_height: None,
                max_height: None,
                padding: [0.0; 4],
                flex_grow: self.flex_grow,
                flex_shrink: self.flex_shrink,
            }),
        );
        if let Some(child) = &self.child {
            // Wrap content in a non-shrinking Box to ensure it overflows the viewport
            // allowing scrolling to work.
            let content_id = cx.next_node_id();
            let mut content_box = NodeBuilder::new(
                content_id,
                Op::Layout(LayoutOp::Box {
                    width: None,
                    height: None,
                    min_width: None,
                    max_width: None,
                    min_height: None,
                    max_height: None,
                    padding: [0.0; 4],
                    flex_grow: 0.0,
                    flex_shrink: 0.0,
                    aspect_ratio: None,
                })
            );
            content_box.add_child(child.lower(cx));
            builder.add_child(content_box.build(cx));
        }

        cx.pop_scope();

        builder.build(cx)
    }
}
