use serde::{Deserialize, Serialize};

use fission_scene::NodeId;

pub const PHYSICS_SNAPSHOT_VERSION: u32 = 1;

/// Stable application-owned identity for one physical body.
#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
#[serde(transparent)]
pub struct PhysicsBodyId(pub u64);

impl PhysicsBodyId {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u64 {
        self.0
    }
}

impl From<NodeId> for PhysicsBodyId {
    fn from(value: NodeId) -> Self {
        Self(value.get())
    }
}

/// How the provider advances a body.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PhysicsBodyKind {
    #[default]
    Dynamic,
    Fixed,
    Kinematic,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ContactKind {
    Contact,
    Trigger,
}

/// Canonically ordered pair of stable body identities.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ContactPair {
    pub first: PhysicsBodyId,
    pub second: PhysicsBodyId,
    pub kind: ContactKind,
}

impl ContactPair {
    pub fn new(first: PhysicsBodyId, second: PhysicsBodyId, kind: ContactKind) -> Self {
        let (first, second) = if first <= second {
            (first, second)
        } else {
            (second, first)
        };
        Self {
            first,
            second,
            kind,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ContactTransition {
    Started,
    Stopped,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContactEvent {
    pub transition: ContactTransition,
    pub pair: ContactPair,
}

/// Provider-neutral spatial-query filtering.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PhysicsQueryFilter {
    pub exclude_body: Option<PhysicsBodyId>,
    pub include_triggers: bool,
}

impl Default for PhysicsQueryFilter {
    fn default() -> Self {
        Self::ALL
    }
}

impl PhysicsQueryFilter {
    pub const ALL: Self = Self {
        exclude_body: None,
        include_triggers: true,
    };

    pub const SOLIDS: Self = Self {
        exclude_body: None,
        include_triggers: false,
    };

    pub const fn excluding(mut self, body: PhysicsBodyId) -> Self {
        self.exclude_body = Some(body);
        self
    }
}
