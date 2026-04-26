pub mod node_id;
pub mod op;
pub mod semantics;
pub mod widget_id;

use serde::{Deserialize, Serialize};
use std::any::Any;
use std::collections::HashMap;
use std::sync::Arc;

pub use node_id::NodeId;
pub use op::{
    AlignItems, EmbedKind, FlexDirection, FlexWrap, GridPlacement, GridTrack, JustifyContent,
    LayoutOp, Op, PaintOp, StructuralOp,
};
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

/// A type-erased render object stored alongside IR nodes.
///
/// Downstream crates (e.g. `fission-core`) store concrete trait objects here
/// (typically `Arc<dyn CustomRenderObject>`).  `fission-ir` itself never
/// inspects these values -- it only provides the storage.
pub type AnyRenderObject = Arc<dyn Any + Send + Sync>;

#[derive(Clone, Serialize, Deserialize)]
pub struct CoreIR {
    pub nodes: HashMap<NodeId, CoreNode>,
    pub root: Option<NodeId>,
    /// Per-node custom render objects.  Keyed by the wrapper `NodeId` created
    /// during lowering of a `CustomNode`.  Skipped by serde because the
    /// concrete trait objects are not serialisable.
    #[serde(skip)]
    pub custom_render_objects: HashMap<NodeId, AnyRenderObject>,
}

impl std::fmt::Debug for CoreIR {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CoreIR")
            .field("nodes", &self.nodes)
            .field("root", &self.root)
            .field(
                "custom_render_objects",
                &format!("({} entries)", self.custom_render_objects.len()),
            )
            .finish()
    }
}

impl PartialEq for CoreIR {
    fn eq(&self, other: &Self) -> bool {
        // custom_render_objects are intentionally excluded from equality --
        // they are ephemeral, non-serialisable extensions.
        self.nodes == other.nodes && self.root == other.root
    }
}

impl Default for CoreIR {
    fn default() -> Self {
        Self {
            nodes: HashMap::new(),
            root: None,
            custom_render_objects: HashMap::new(),
        }
    }
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
