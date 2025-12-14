use serde::{Deserialize, Serialize};
use crate::lowering::LoweringContext;
use fission_ir::{
    op::{LayoutOp, Op, FlexDirection},
    NodeId, Semantics
};
use crate::ui::Node;
use crate::ui::traits::Lower;

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Row {
    pub id: Option<NodeId>,
    pub children: Vec<Node>,
    pub semantics: Option<Semantics>,
    pub flex_grow: f32,
    pub flex_shrink: f32,
}

impl Lower for Row {
    fn lower(&self, cx: &mut LoweringContext) -> NodeId {
        let mut child_ids = Vec::new();
        for child in &self.children {
            child_ids.push(child.lower(cx));
        }

        let layout_id = self.id.unwrap_or_else(|| cx.next_node_id());

        cx.add_node(
            layout_id,
            Op::Layout(LayoutOp::Flex {
                direction: FlexDirection::Row,
                flex_grow: self.flex_grow,
                flex_shrink: self.flex_shrink,
                padding: [0.0; 4],
            }),
            child_ids,
        );

        if let Some(s) = &self.semantics {
            let semantics_id = cx.next_node_id();
            cx.add_node(semantics_id, Op::Semantics(s.clone()), vec![layout_id]);
            return semantics_id;
        }

        layout_id
    }
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Column {
    pub id: Option<NodeId>,
    pub children: Vec<Node>,
    pub semantics: Option<Semantics>,
    pub flex_grow: f32,
    pub flex_shrink: f32,
}

impl Lower for Column {
    fn lower(&self, cx: &mut LoweringContext) -> NodeId {
        let mut child_ids = Vec::new();
        for child in &self.children {
            child_ids.push(child.lower(cx));
        }

        let layout_id = self.id.unwrap_or_else(|| cx.next_node_id());

        cx.add_node(
            layout_id,
            Op::Layout(LayoutOp::Flex {
                direction: FlexDirection::Column,
                flex_grow: self.flex_grow,
                flex_shrink: self.flex_shrink,
                padding: [0.0; 4],
            }),
            child_ids,
        );

        if let Some(s) = &self.semantics {
            let semantics_id = cx.next_node_id();
            cx.add_node(semantics_id, Op::Semantics(s.clone()), vec![layout_id]);
            return semantics_id;
        }

        layout_id
    }
}
