use crate::{Lower, LoweringContext, Node, NodeBuilder};
use fission_ir::{FlexDirection, LayoutOp, NodeId, Op, Semantics};
use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Column {
    pub id: Option<NodeId>,
    pub children: Vec<Node>,
    pub semantics: Option<Semantics>,
    pub flex_grow: f32,
    pub flex_shrink: f32,
    pub gap: Option<f32>,
}

impl Lower for Column {
    fn lower(&self, cx: &mut LoweringContext) -> NodeId {
        let layout_id = self.id.unwrap_or_else(|| cx.next_node_id());
        
        cx.push_scope(layout_id);

        let mut builder = NodeBuilder::new(
            layout_id,
            Op::Layout(LayoutOp::Flex {
                direction: FlexDirection::Column,
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
            // Semantics wrap the layout node, but conceptually they are "above" it.
            // However, since they are generated *after* layout_id is consumed, 
            // their ID should be generated in the *parent* scope (which it is, since we popped).
            let mut semantics_builder =
                NodeBuilder::new(cx.next_node_id(), Op::Semantics(s.clone()));
            semantics_builder.add_child(layout_id);
            return semantics_builder.build(cx);
        }

        layout_id
    }
}
