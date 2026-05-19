//! Actions, envelopes, and application state traits.
//!
//! This module defines the core data-flow primitives:
//!
//! - [`Action`] -- a strongly-typed, serialisable event payload.
//! - [`ActionEnvelope`] -- the type-erased transport format dispatched through
//!   the [`Runtime`](crate::Runtime).
//! - [`ActionId`] -- a stable, content-addressed identifier derived from the
//!   action's type name.
//! - [`AppState`] -- trait for application state managed by the runtime.

use blake3;
use downcast_rs::{impl_downcast, Downcast};
use fission_ir::NodeId;
// use fission_macros::Action;
use lazy_static::lazy_static;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json;
use std::any::Any;

pub mod video;

pub use video::{
    VideoPause, VideoPlay, VideoSeek, VideoSetMuted, VideoSetRate, VideoSetVolume, VideoStop,
};

/// Built-in action to trigger an undo operation.
///
/// Applications that support undo/redo should register a reducer for this
/// action on their state type.
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

/// Built-in action to trigger a redo operation.
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

/// A stable, globally unique identifier for an [`Action`] type.
///
/// `ActionId` is computed as the first 128 bits of a BLAKE3 hash of the
/// action's fully-qualified type name, making it deterministic across
/// compilations and platforms.
///
/// # Example
///
/// ```rust,ignore
/// let id = ActionId::from_name("my_app::IncrementCounter");
/// assert_eq!(id, ActionId::from_name("my_app::IncrementCounter")); // stable
/// ```
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize, PartialOrd, Ord)]
pub struct ActionId(u128);

impl ActionId {
    /// Creates an `ActionId` from a raw `u128` value.
    pub const fn from_u128(val: u128) -> Self {
        Self(val)
    }

    /// Returns the underlying `u128` value.
    pub fn as_u128(&self) -> u128 {
        self.0
    }

    /// Derives a deterministic `ActionId` from a human-readable name string.
    ///
    /// The name is hashed with BLAKE3; the first 16 bytes become the id.
    pub fn from_name(name: &str) -> Self {
        let mut hasher = blake3::Hasher::new();
        hasher.update(name.as_bytes());
        let hash = hasher.finalize();
        ActionId(u128::from_le_bytes(
            hash.as_bytes()[0..16].try_into().unwrap(),
        ))
    }
}

/// A stable scope identifier for raw action dispatch.
///
/// Scopes let a host register raw handlers for action IDs that are meaningful
/// only inside a mounted subtree. The envelope remains unchanged; dispatch
/// carries the nearest enclosing scope in [`ActionInput`](crate::ActionInput).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize, PartialOrd, Ord)]
pub struct ActionScopeId(u128);

impl ActionScopeId {
    /// Creates an `ActionScopeId` from a raw `u128` value.
    pub const fn from_u128(val: u128) -> Self {
        Self(val)
    }

    /// Returns the underlying `u128` value.
    pub fn as_u128(&self) -> u128 {
        self.0
    }

    /// Derives a deterministic `ActionScopeId` from a stable name.
    pub fn from_name(name: &str) -> Self {
        let mut hasher = blake3::Hasher::new();
        hasher.update(b"fission.action_scope.v1:");
        hasher.update(name.as_bytes());
        let hash = hasher.finalize();
        ActionScopeId(u128::from_le_bytes(
            hash.as_bytes()[0..16].try_into().unwrap(),
        ))
    }
}

/// Action dispatched by the text-editing controller when the user modifies a
/// [`TextInput`](crate::ui::TextInput) field.
///
/// Contains the full new text and updated caret/selection positions.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UpdateTextInput {
    /// The IR node id of the text input that changed.
    pub node_id: NodeId,
    /// The complete new text value.
    pub new_text: String,
    /// Byte offset of the caret (insertion point).
    pub new_caret: usize,
    /// Byte offset of the selection anchor (equals `new_caret` when no
    /// selection is active).
    pub new_anchor: usize,
}

impl Action for UpdateTextInput {
    fn static_id() -> ActionId {
        lazy_static! {
            pub static ref UPDATE_TEXT_INPUT_ACTION_ID: ActionId =
                ActionId::from_name("fission_core::UpdateTextInput");
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
            pub static ref CURSOR_CHANGED_ACTION_ID: ActionId =
                ActionId::from_name("fission_core::CursorChanged");
        }
        *CURSOR_CHANGED_ACTION_ID
    }
}

/// A strongly-typed, serialisable event payload.
///
/// Every action type must be `Serialize + DeserializeOwned + Send + Sync + Debug`
/// and provide a stable [`ActionId`] via [`Action::static_id`]. The runtime
/// uses JSON serialisation internally, so actions travel across the
/// widget/reducer boundary without generics.
///
/// # Implementing `Action`
///
/// ```rust,ignore
/// use fission_core::{Action, ActionId};
/// use serde::{Deserialize, Serialize};
///
/// #[derive(Debug, Clone, Serialize, Deserialize)]
/// struct SetName { name: String }
///
/// impl Action for SetName {
///     fn static_id() -> ActionId {
///         ActionId::from_name("my_app::SetName")
///     }
/// }
/// ```
pub trait Action: Serialize + DeserializeOwned + Any + Send + Sync + std::fmt::Debug {
    /// Returns the globally unique, deterministic identifier for this action type.
    fn static_id() -> ActionId
    where
        Self: Sized;

    /// Serialises the action to JSON bytes for transport inside an
    /// [`ActionEnvelope`].
    fn encode(&self) -> Vec<u8> {
        serde_json::to_vec(self).expect("Action serialization failed")
    }
}

/// A type-erased action envelope that can be stored in widget trees and
/// dispatched through the [`Runtime`](crate::Runtime).
///
/// `ActionEnvelope` pairs an [`ActionId`] with opaque JSON bytes so that the
/// reducer pipeline can route and deserialise actions without compile-time
/// knowledge of the concrete type.
///
/// # Creating an envelope
///
/// ```rust,ignore
/// let envelope: ActionEnvelope = my_action.into();
/// runtime.dispatch(envelope, target_node)?;
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActionEnvelope {
    /// The identifier that routes this envelope to the correct reducer(s).
    pub id: ActionId,
    /// Opaque JSON-serialised payload bytes.
    pub payload: Vec<u8>,
}

/// A typed wrapper around an [`Action`] value that converts into an
/// [`ActionEnvelope`] via `From`.
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

/// Trait for application state managed by the [`Runtime`](crate::Runtime).
///
/// Any type that is `Send + Sync + Debug + 'static` can serve as application
/// state. The runtime stores at most one instance of each concrete type.
///
/// # Example
///
/// ```rust,ignore
/// #[derive(Debug, Default)]
/// struct TodoList {
///     items: Vec<String>,
/// }
/// impl AppState for TodoList {}
///
/// // Register with the runtime:
/// runtime.add_app_state(Box::new(TodoList::default()))?;
/// ```
pub trait AppState: Any + Send + Sync + std::fmt::Debug + Downcast {}

impl_downcast!(AppState);

/// Type alias for the legacy 3-argument reducer signature used by
/// [`Runtime::register_reducer`](crate::Runtime::register_reducer).
///
/// Prefer the modern handler signature via [`BuildCtx::bind`](crate::BuildCtx::bind) which
/// provides access to effects and input context.
pub type Reducer<S> = fn(&mut S, &ActionEnvelope, NodeId) -> anyhow::Result<()>;
