use crate::{Lower, LoweringContext, Node, NodeBuilder};
use fission_ir::{FlexDirection, LayoutOp, NodeId, Op, Semantics};
use fission_ir::op::FlexWrap;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Column {
    pub id: Option<NodeId>,
    pub children: Vec<Node>,
    pub semantics: Option<Semantics>,
    pub flex_grow: f32,
    pub flex_shrink: f32,
    pub gap: Option<f32>,
    pub wrap: FlexWrap,
}

impl Default for Column {
    fn default() -> Self {
        Self {
            id: None,
            children: Vec::new(),
            semantics: None,
            flex_grow: 0.0,
            flex_shrink: 0.0,
            gap: None,
            wrap: FlexWrap::NoWrap,
        }
    }
}

impl Column {
    pub fn into_node(self) -> Node {
        Node::Column(self)
    }
}

impl Lower for Column {
    fn lower(&self, cx: &mut LoweringContext) -> NodeId {
        let layout_id = self.id.unwrap_or_else(|| cx.next_node_id());
        
        cx.push_scope(layout_id);
        
        let mut builder = NodeBuilder::new(
            layout_id,
            Op::Layout(LayoutOp::Flex {
                direction: FlexDirection::Column,
                wrap: self.wrap,
                flex_grow: self.flex_grow,
                flex_shrink: self.flex_shrink,
                padding: [0.0; 4],
                gap: self.gap,
            }),
        );
        for child in &self.children {
            builder.add_child(child.lower(cx));
        }
        
        cx.pop_scope();
        
        let layout_id = builder.build(cx);

        if let Some(s) = &self.semantics {
            let mut semantics_builder =
                NodeBuilder::new(cx.next_node_id(), Op::Semantics(s.clone()));
            semantics_builder.add_child(layout_id);
            return semantics_builder.build(cx);
        }

        layout_id
    }
}
