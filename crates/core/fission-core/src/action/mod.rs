use blake3;
use downcast_rs::{impl_downcast, Downcast};
use fission_ir::NodeId;
use fission_macros::Action;
use lazy_static::lazy_static;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json;
use std::any::Any;

pub mod video;

pub use video::{VideoPause, VideoPlay};

// ActionId is a stable, globally unique identifier for an Action type.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize, PartialOrd, Ord)]
pub struct ActionId(u128);

impl ActionId {
    pub const fn from_u128(val: u128) -> Self {
        Self(val)
    }

    pub fn as_u128(&self) -> u128 {
        self.0
    }

    pub fn from_name(name: &str) -> Self {
        let mut hasher = blake3::Hasher::new();
        hasher.update(name.as_bytes());
        let hash = hasher.finalize();
        ActionId(u128::from_le_bytes(
            hash.as_bytes()[0..16].try_into().unwrap(),
        ))
    }
}

// The Action trait for typed authoring.
// Must be Serializable/Deserializable to support the Envelope model.
pub trait Action: Serialize + DeserializeOwned + Any + Send + Sync + std::fmt::Debug {
    fn static_id() -> ActionId
    where
        Self: Sized;

    fn encode(&self) -> Vec<u8> {
        serde_json::to_vec(self).expect("Action serialization failed")
    }
}

// The type-erased envelope stored in widgets and passed to reducers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActionEnvelope {
    pub id: ActionId,
    // Payload is opaque bytes. serde_bytes could be used for optimization but Vec<u8> is fine for MVP.
    pub payload: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActionRef<T: Action>(pub T);

impl<T: Action> From<ActionRef<T>> for ActionEnvelope {
    fn from(action_ref: ActionRef<T>) -> Self {
        ActionEnvelope {
            id: T::static_id(),
            payload: action_ref.0.encode(),
        }
    }
}

// Also allow direct conversion for convenience if desired?
impl<T: Action> From<T> for ActionEnvelope {
    fn from(action: T) -> Self {
        ActionEnvelope {
            id: T::static_id(),
            payload: action.encode(),
        }
    }
}

// Trait for application state that can be managed by the Runtime.
pub trait AppState: Any + Send + Sync + std::fmt::Debug + Downcast {}

impl_downcast!(AppState);

pub type Reducer<S> = fn(&mut S, &ActionEnvelope, NodeId) -> anyhow::Result<()>;
