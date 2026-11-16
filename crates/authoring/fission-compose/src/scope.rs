use crate::core::GlobalState;
use serde::{de::DeserializeOwned, Serialize};
use std::fmt::Debug;

/// A read/write projection. `None` means the target has disappeared; never insert it
/// just because a stale widget event arrived. Keys should include an entity generation
/// when IDs can be recycled within the lifetime of an open editor.
pub trait Scope: Send + Sync + 'static {
    type Root: GlobalState;
    type Target: Send + Sync + 'static;
    type Key: crate::identity::UiKey
        + Clone
        + PartialEq
        + Debug
        + Serialize
        + DeserializeOwned
        + Send
        + Sync
        + 'static;
    fn get<'a>(root: &'a Self::Root, key: &Self::Key) -> Option<&'a Self::Target>;
    fn get_mut<'a>(root: &'a mut Self::Root, key: &Self::Key) -> Option<&'a mut Self::Target>;
}

/// An optional default, declared by the application rather than guessed by type reflection.
pub trait DefaultScope: Sized + Send + Sync + 'static {
    type Scope: Scope<Target = Self, Key = ()>;
}
