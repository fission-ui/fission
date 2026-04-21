use blake3;
use downcast_rs::{impl_downcast, Downcast};
use fission_ir::NodeId;
// use fission_macros::Action;
use lazy_static::lazy_static;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json;
use std::any::Any;

use crate as fission_core;

pub mod video;

pub use video::{
    VideoPause, VideoPlay, VideoSeek, VideoSetMuted, VideoSetRate, VideoSetVolume, VideoStop,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Undo;

impl Action for Undo {
    fn static_id() -> ActionId {
        lazy_static! {
            pub static ref UNDO_ACTION_ID: ActionId = ActionId::from_name("fission_core::Undo");
        }
        *UNDO_ACTION_ID
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Redo;

impl Action for Redo {
    fn static_id() -> ActionId {
        lazy_static! {
            pub static ref REDO_ACTION_ID: ActionId = ActionId::from_name("fission_core::Redo");
        }
        *REDO_ACTION_ID
    }
}

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


#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UpdateTextInput {
    pub node_id: NodeId,
    pub new_text: String,
    pub new_caret: usize,
    pub new_anchor: usize,
}

impl Action for UpdateTextInput {
    fn static_id() -> ActionId {
        lazy_static! {
            pub static ref UPDATE_TEXT_INPUT_ACTION_ID: ActionId = ActionId::from_name("fission_core::UpdateTextInput");
        }
        *UPDATE_TEXT_INPUT_ACTION_ID
    }
}

/// Payload dispatched when the caret/anchor position changes in a TextInput.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CursorChanged {
    pub caret: usize,
    pub anchor: usize,
}

impl Action for CursorChanged {
    fn static_id() -> ActionId {
        lazy_static! {
            pub static ref CURSOR_CHANGED_ACTION_ID: ActionId = ActionId::from_name("fission_core::CursorChanged");
        }
        *CURSOR_CHANGED_ACTION_ID
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
