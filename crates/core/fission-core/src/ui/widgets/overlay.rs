use crate::lowering::{LoweringContext, NodeBuilder, wrap_zstack_child};
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

impl Overlay {
    pub fn into_node(self) -> Node {
        Node::Overlay(self)
    }
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

        // Build overlay child in its own scope to avoid ID collisions
        // with the content tree.
        let overlay_scope = cx.next_node_id();
        cx.push_scope(overlay_scope);
        let overlay_child_id = self.overlay.lower(cx);
        cx.pop_scope();
        let mut overlay_fill =
            NodeBuilder::new(cx.next_node_id(), Op::Layout(LayoutOp::AbsoluteFill));
        overlay_fill.add_child(overlay_child_id);
        let overlay_fill_id = overlay_fill.build(cx);

        // Stack container: content first, overlay second.
        let stack_id = cx.next_node_id();
        let content_id = self.content.lower(cx);
        cx.push_scope(stack_id);
        let content_wrapped = wrap_zstack_child(cx, content_id);
        let overlay_wrapped = wrap_zstack_child(cx, overlay_fill_id);
        cx.pop_scope();

        let mut stack = NodeBuilder::new(stack_id, Op::Layout(LayoutOp::ZStack));
        stack.add_child(content_wrapped);
        stack.add_child(overlay_wrapped);
        let stack_id = stack.build(cx);

        // Ensure the stack fills available space so overlay AbsoluteFill can cover
        // the full viewport even when content is small.
        let mut stack_wrapper = NodeBuilder::new(
            cx.next_node_id(),
            Op::Layout(LayoutOp::Box {
                width: None,
                height: None,
                min_width: None,
                max_width: None,
                min_height: None,
                max_height: None,
                padding: [0.0; 4],
                flex_grow: 1.0,
                flex_shrink: 1.0,
                aspect_ratio: None,
            }),
        );
        stack_wrapper.add_child(stack_id);
        let stack_wrapper_id = stack_wrapper.build(cx);

        // Wrap ZStack in a Flex container with flex_grow = 1.0
        // Flex defaults to stretching children, unlike Box which centers.
        let mut root = NodeBuilder::new(
            id, 
            Op::Layout(LayoutOp::Flex {
                direction: fission_ir::FlexDirection::Column,
                wrap: fission_ir::FlexWrap::NoWrap,
                flex_grow: 1.0,
                flex_shrink: 1.0,
                padding: [0.0; 4],
                gap: None,
                align_items: fission_ir::op::AlignItems::Stretch,
                justify_content: fission_ir::op::JustifyContent::Start,
            })
        );
        root.add_child(stack_wrapper_id);
        
        cx.pop_scope();
        
        root.build(cx)
    }
}
