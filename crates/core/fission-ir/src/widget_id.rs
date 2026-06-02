//! Stable identity for widgets and lowered IR nodes.
//!
//! A [`WidgetId`] is the single identity type used across authoring widgets,
//! lowered IR nodes, layout, rendering, hit testing, and runtime state. The old
//! split between widget identity and node identity is intentionally gone: a
//! widget may lower to one or more IR nodes, and those nodes use derived
//! `WidgetId` values when they need child identities.

use serde::{Deserialize, Serialize};
use std::fmt;

/// A stable 128-bit identity for widgets and lowered IR nodes.
///
/// `WidgetId` values are derived from BLAKE3 hashes. Two construction strategies
/// are available:
///
/// * [`WidgetId::explicit`] hashes a user-provided stable key.
/// * [`WidgetId::derived`] hashes a parent identity plus a child-index path.
///
/// # Example
///
/// ```rust
/// use fission_ir::WidgetId;
///
/// let sidebar = WidgetId::explicit("sidebar");
/// let first_item = WidgetId::derived(sidebar.as_u128(), &[0]);
/// assert_ne!(sidebar, first_item);
/// ```
#[derive(Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, PartialOrd, Ord)]
pub struct WidgetId(u128);

impl WidgetId {
    /// Creates a `WidgetId` from a raw 128-bit value.
    ///
    /// This is intended for internal use or deserialization. In normal code use
    /// [`WidgetId::explicit`] or [`WidgetId::derived`] instead.
    pub const fn from_u128(val: u128) -> Self {
        Self(val)
    }

    /// Returns the underlying 128-bit value.
    pub fn as_u128(&self) -> u128 {
        self.0
    }

    /// Creates a `WidgetId` from a user-provided stable key.
    ///
    /// The key is hashed with BLAKE3 using the same explicit-identity domain as
    /// the original IR identity system. Keep the key stable across rebuilds when
    /// you want runtime state, focus, scroll, animation, or host-surface state to
    /// follow a widget through tree changes.
    pub fn explicit(key: &str) -> Self {
        let mut hasher = blake3::Hasher::new();
        hasher.update(b"explicit:");
        hasher.update(key.as_bytes());
        let hash = hasher.finalize();
        Self(u128::from_le_bytes(
            hash.as_bytes()[0..16].try_into().unwrap(),
        ))
    }

    /// Creates a `WidgetId` derived from a parent identity and child-index path.
    ///
    /// This provides structural identity for children that do not have explicit
    /// keys. Dynamic/reorderable lists should provide explicit IDs for list items;
    /// purely structural children can use derived IDs.
    pub fn derived(parent: u128, path: &[u32]) -> Self {
        let mut hasher = blake3::Hasher::new();
        hasher.update(b"derived:");
        hasher.update(&parent.to_le_bytes());
        for index in path {
            hasher.update(&index.to_le_bytes());
        }
        let hash = hasher.finalize();
        Self(u128::from_le_bytes(
            hash.as_bytes()[0..16].try_into().unwrap(),
        ))
    }
}

impl fmt::Debug for WidgetId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "WidgetId({:032x})", self.0)
    }
}

impl fmt::Display for WidgetId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:032x}", self.0)
    }
}
