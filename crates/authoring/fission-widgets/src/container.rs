use std::sync::Arc;

use fission_core::{
    action::AppState,
    op::{Fill, LayoutOp, Op, PaintOp, Stroke},
    ui::{CustomNode, Node, Text, TextContent, Widget},
    BuildCtx, Color, LowerDyn, LoweringContext, NodeBuilder, NodeId, View,
};

#[derive(Default, Clone, Debug)]
pub struct Container {
    pub child: Option<Node>,
    pub background: Option<Color>,
    pub border_color: Option<Color>,
    pub border_width: f32,
}

impl Container {
    pub fn new(child: Node) -> Self {
        Self {
            child: Some(child),
            ..Default::default()
        }
    }

    pub fn background(mut self, color: Color) -> Self {
        self.background = Some(color);
        self
    }

    pub fn border(mut self, color: Color, width: f32) -> Self {
        self.border_color = Some(color);
        self.border_width = width;
        self
    }

    pub fn into_node(self) -> Node {
        Node::Custom(CustomNode {
            debug_tag: "Container".into(),
            lowerer: Some(Arc::new(self)),
        })
    }
}

impl LowerDyn for Container {
    fn lower_dyn(&self, cx: &mut LoweringContext) -> NodeId {
        let mut children = vec![];

        if self.background.is_some() || self.border_color.is_some() {
            let paint_op = Op::Paint(PaintOp::DrawRect {
                fill: self.background.map(|c| Fill { color: c }),
                stroke: self.border_color
                    .map(|c| Stroke { color: c, width: self.border_width }),
                corner_radius: 0.0,
                shadow: None,
            });
            children.push(NodeBuilder::new(cx.next_node_id(), paint_op).build(cx));
        }

        if let Some(child) = self.child.clone() {
            children.push(child.lower(cx));
        }

        let stack_op = Op::Layout(LayoutOp::ZStack);
        let mut stack_builder = NodeBuilder::new(cx.next_node_id(), stack_op);
        for child_id in children {
            stack_builder.add_child(child_id);
        }
        stack_builder.build(cx)
    }
}
