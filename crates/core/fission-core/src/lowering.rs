use crate::env::{Env, RuntimeState};
use blake3;
use fission_ir::{CoreIR, FlexDirection, LayoutOp, NodeId, Op, PaintOp, WidgetNodeId};
use fission_layout::{LayoutInputNode, LayoutPoint, LayoutSize, LayoutUnit};
use serde_json;
use std::collections::HashMap;
use std::fmt::Debug;

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

    pub fn widget_node_id(&self, widget_id: WidgetNodeId) -> NodeId {
        widget_id.into()
    }

    fn insert_node(&mut self, node_id: NodeId, op: Op, children: Vec<NodeId>) {
        let mut hasher = blake3::Hasher::new();
        // Hash Op
        if let Ok(op_bytes) = serde_json::to_vec(&op) {
            hasher.update(&op_bytes);
        } else {
            panic!("Failed to serialize op for hashing: {:?}", op);
        }

        // Hash Children Hashes (Merkle)
        for child_id in &children {
            if let Some(child) = self.ir.nodes.get(child_id) {
                hasher.update(&child.hash.to_le_bytes());
            } else {
            }
        }

        let hash_bytes = hasher.finalize();
        let hash = u64::from_le_bytes(hash_bytes.as_bytes()[0..8].try_into().unwrap());

        self.ir.add_node(node_id, op, children);

        if let Some(node) = self.ir.nodes.get_mut(&node_id) {
            node.hash = hash;
        }
    }
}

pub struct NodeBuilder {
    node_id: NodeId,
    op: Op,
    children: Vec<NodeId>,
}

impl NodeBuilder {
    pub fn new(node_id: NodeId, op: Op) -> Self {
        Self {
            node_id,
            op,
            children: Vec::new(),
        }
    }

    pub fn add_child(&mut self, child: NodeId) {
        self.children.push(child);
    }

    pub fn add_children<I>(&mut self, children: I)
    where
        I: IntoIterator<Item = NodeId>,
    {
        self.children.extend(children);
    }

    pub fn build(self, cx: &mut LoweringContext) -> NodeId {
        cx.insert_node(self.node_id, self.op, self.children);
        self.node_id
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
        let mut text_content = None;
        let mut font_size = None;

        let (layout_op_variant, width, height, flex_grow, flex_shrink) = match &node.op {
            Op::Layout(LayoutOp::Box {
                width,
                height,
                padding,
            }) => (
                LayoutOp::Box {
                    width: *width,
                    height: *height,
                    padding: *padding,
                },
                *width,
                *height,
                0.0,
                0.0,
            ),
            Op::Layout(LayoutOp::Flex {
                direction,
                flex_grow,
                flex_shrink,
                padding,
            }) => (
                LayoutOp::Flex {
                    direction: *direction,
                    flex_grow: *flex_grow,
                    flex_shrink: *flex_shrink,
                    padding: *padding,
                },
                None,
                None,
                *flex_grow,
                *flex_shrink,
            ),
            Op::Layout(LayoutOp::Scroll {
                direction,
                show_scrollbar,
                width,
                height,
                padding,
            }) => (
                LayoutOp::Scroll {
                    direction: *direction,
                    show_scrollbar: *show_scrollbar,
                    width: *width,
                    height: *height,
                    padding: *padding,
                },
                *width,
                *height,
                0.0,
                0.0,
            ),
            Op::Layout(LayoutOp::Embed { kind, widget_id }) => {
                let mut width = None;
                let mut height = None;
                if let Some(parent_id) = parent_map.get(id) {
                    if let Some(parent_node) = ir.nodes.get(parent_id) {
                        if let Op::Layout(LayoutOp::Box {
                            width: w,
                            height: h,
                            ..
                        }) = parent_node.op
                        {
                            width = w;
                            height = h;
                        }
                    }
                }

                (
                    LayoutOp::Embed {
                        kind: *kind,
                        widget_id: *widget_id,
                    },
                    width,
                    height,
                    0.0,
                    0.0,
                )
            }

            Op::Paint(PaintOp::DrawText { text, size, .. }) => {
                text_content = Some(text.clone());
                font_size = Some(*size);
                (
                    LayoutOp::Box {
                        width: None,
                        height: None,
                        padding: [0.0; 4],
                    },
                    None,
                    None,
                    0.0,
                    0.0,
                )
            }

            Op::Paint(PaintOp::DrawImage { .. }) => (LayoutOp::AbsoluteFill, None, None, 0.0, 0.0),

            Op::Paint(_) => (LayoutOp::AbsoluteFill, None, None, 0.0, 0.0),
            _ => (
                LayoutOp::Box {
                    width: None,
                    height: None,
                    padding: [0.0; 4],
                },
                None,
                None,
                0.0,
                0.0,
            ),
        };

        input_nodes.push(LayoutInputNode {
            id: *id,
            parent_id: parent_map.get(id).copied(),
            op: layout_op_variant,
            children_ids: node.children.clone(),
            debug_name: format!("{:?}", node.id),
            width,
            height,
            flex_grow,
            flex_shrink,
            text_content,
            font_size,
        });
    }

    input_nodes
}
