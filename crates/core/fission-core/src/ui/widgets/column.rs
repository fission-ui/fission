use crate::{Lower, LoweringContext, Node, NodeBuilder};
use fission_ir::{FlexDirection, LayoutOp, NodeId, Op, Semantics};
use fission_ir::op::{FlexWrap, AlignItems, JustifyContent};
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
    pub align_items: AlignItems,
    pub justify_content: JustifyContent,
}

impl Default for Column {
    fn default() -> Self {
        Self {
            id: None,
            children: Vec::new(),
            gap: None,
            flex_grow: 0.0,
            flex_shrink: 1.0,
            semantics: None,
            wrap: FlexWrap::NoWrap,
            align_items: AlignItems::Stretch,
            justify_content: JustifyContent::Start,
        }
    }
}

impl Column {
    pub fn children(mut self, children: Vec<Node>) -> Self {
        self.children = children;
        self
    }

    pub fn flex_grow(mut self, flex_grow: f32) -> Self {
        self.flex_grow = flex_grow;
        self
    }

    pub fn gap(mut self, gap: Option<f32>) -> Self {
        self.gap = gap;
        self
    }

    pub fn align_items(mut self, align: AlignItems) -> Self {
        self.align_items = align;
        self
    }

    pub fn justify_content(mut self, justify: JustifyContent) -> Self {
        self.justify_content = justify;
        self
    }

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
                align_items: self.align_items,
                justify_content: self.justify_content,
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
