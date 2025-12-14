use fission_ir::{NodeId, Op, CoreIR, LayoutOp, FlexDirection};
use fission_layout::{
    LayoutInputNode, LayoutPoint, LayoutSize, LayoutUnit
};
use crate::env::{Env, RuntimeState}; 
use std::fmt::Debug;
use std::collections::HashMap;

// Context passed down during the lowering phase.
pub struct LoweringContext<'a> {
    pub next_node_id_seed: u128,
    pub ir: CoreIR,
    pub env: &'a Env,
    pub runtime_state: &'a RuntimeState,
}

impl<'a> LoweringContext<'a> {
    pub fn new(env: &'a Env, runtime_state: &'a RuntimeState) -> Self {
        LoweringContext {
            next_node_id_seed: 0,
            ir: CoreIR::new(),
            env,
            runtime_state,
        }
    }

    pub fn next_node_id(&mut self) -> NodeId {
        self.next_node_id_seed += 1;
        NodeId::derived(0, &[self.next_node_id_seed as u32])
    }

    pub fn add_node(&mut self, node_id: NodeId, op: Op, children: Vec<NodeId>) {
        self.ir.add_node(node_id, op, children);
    }
}

pub fn build_layout_tree(ir: &CoreIR) -> Vec<LayoutInputNode> {
    let mut input_nodes = Vec::new();
    
    let mut parent_map = HashMap::new();
    for (id, node) in &ir.nodes {
        for child in &node.children {
            parent_map.insert(*child, *id);
        }
    }
    
    for (id, node) in &ir.nodes {
        let (layout_op_variant, width, height, flex_grow, flex_shrink) = match &node.op {
            Op::Layout(LayoutOp::Box { width, height }) => (LayoutOp::Box { width: *width, height: *height }, *width, *height, 0.0, 0.0),
            Op::Layout(LayoutOp::Flex { direction, flex_grow, flex_shrink }) => (LayoutOp::Flex { direction: *direction, flex_grow: *flex_grow, flex_shrink: *flex_shrink }, None, None, *flex_grow, *flex_shrink),
            Op::Paint(_) => (LayoutOp::AbsoluteFill, None, None, 0.0, 0.0), // Paint nodes fill parent
            _ => (LayoutOp::Box { width: None, height: None }, None, None, 0.0, 0.0), 
        };
        
        input_nodes.push(LayoutInputNode {
            id: *id,
            parent_id: parent_map.get(id).copied(),
            op: layout_op_variant, // Pass the extracted LayoutOp variant
            children_ids: node.children.clone(),
            debug_name: format!("{:?}", node.id),
            width,
            height,
            flex_grow,
            flex_shrink,
        });
    }

    input_nodes
}
