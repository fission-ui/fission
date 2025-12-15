pub mod node_id;
pub mod op;
pub mod semantics;
pub mod widget_id;

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub use node_id::NodeId;
pub use op::{Color, EmbedKind, FlexDirection, LayoutOp, Op, PaintOp, StructuralOp};
pub use semantics::{ActionEntry, ActionSet, Role, Semantics};
pub use widget_id::WidgetNodeId;

pub const IR_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CoreNode {
    pub id: NodeId,
    pub op: Op,
    pub children: Vec<NodeId>,
    pub parent: Option<NodeId>,
    pub hash: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct CoreIR {
    pub nodes: HashMap<NodeId, CoreNode>,
    pub root: Option<NodeId>,
}

impl CoreIR {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_node(&mut self, id: NodeId, op: Op, children: Vec<NodeId>) {
        let mut core_node = CoreNode {
            id,
            op,
            children: children.clone(),
            parent: None,
            hash: 0,
        };
        self.nodes.insert(id, core_node);

        for child_id in children {
            if let Some(child_node) = self.nodes.get_mut(&child_id) {
                child_node.parent = Some(id);
            }
        }
    }

    pub fn set_root(&mut self, id: NodeId) {
        self.root = Some(id);
    }
}
