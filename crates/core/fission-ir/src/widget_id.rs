use blake3;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, PartialOrd, Ord, Debug)]
pub struct WidgetNodeId(u128);

impl WidgetNodeId {
    pub const fn from_u128(val: u128) -> Self {
        Self(val)
    }

    pub fn as_u128(&self) -> u128 {
        self.0
    }

    pub fn explicit(name: &str) -> Self {
        let mut hasher = blake3::Hasher::new();
        hasher.update(b"widget:");
        hasher.update(name.as_bytes());
        let hash = hasher.finalize();
        Self(u128::from_le_bytes(
            hash.as_bytes()[0..16].try_into().unwrap(),
        ))
    }
}

impl From<crate::node_id::NodeId> for WidgetNodeId {
    fn from(node: crate::node_id::NodeId) -> Self {
        Self(node.as_u128())
    }
}

impl From<WidgetNodeId> for crate::node_id::NodeId {
    fn from(id: WidgetNodeId) -> Self {
        crate::node_id::NodeId::from_u128(id.0)
    }
}
