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
/// Fission assigns every widget a deterministic identity from the application
/// root and its structural child path. Most application code therefore does
/// not need to set IDs. Unidentified collection items intentionally retain
/// identity by position; give each logical item an explicit ID when state must
/// follow the item through insertion, removal, filtering, or reordering.
///
/// `WidgetId` values are derived from BLAKE3 hashes. Two public construction
/// strategies are available:
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
    /// Returns the deterministic identity seed used for an application's root
    /// widget when the embedding shell does not provide a mount-specific id.
    pub fn app_root() -> Self {
        Self::explicit("fission.app.root")
    }

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

    /// Creates an identity scoped to a parent and a stable string key.
    ///
    /// This is used for authoring locations and keyed collection items where a
    /// numeric child position is not the logical identity authority.
    pub fn scoped(parent: u128, key: &str) -> Self {
        let mut hasher = blake3::Hasher::new();
        hasher.update(b"scoped:");
        hasher.update(&parent.to_le_bytes());
        hasher.update(key.as_bytes());
        let hash = hasher.finalize();
        Self(u128::from_le_bytes(
            hash.as_bytes()[0..16].try_into().unwrap(),
        ))
    }

    /// Creates an identity scoped to a source location without allocating a
    /// temporary location string.
    #[doc(hidden)]
    pub fn scoped_location(parent: u128, file: &str, line: u32, column: u32) -> Self {
        let mut hasher = blake3::Hasher::new();
        hasher.update(b"source-location:");
        hasher.update(&parent.to_le_bytes());
        hasher.update(file.as_bytes());
        hasher.update(&line.to_le_bytes());
        hasher.update(&column.to_le_bytes());
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

#[cfg(test)]
mod tests {
    use super::WidgetId;

    #[test]
    fn all_identity_domains_are_deterministic_and_distinct() {
        let parent = WidgetId::explicit("parent");
        let explicit = WidgetId::explicit("child");
        let derived = WidgetId::derived(parent.as_u128(), &[4, 2]);
        let scoped = WidgetId::scoped(parent.as_u128(), "child");

        assert_eq!(explicit, WidgetId::explicit("child"));
        assert_eq!(derived, WidgetId::derived(parent.as_u128(), &[4, 2]));
        assert_eq!(scoped, WidgetId::scoped(parent.as_u128(), "child"));
        assert_ne!(explicit, derived);
        assert_ne!(explicit, scoped);
        assert_ne!(derived, scoped);
    }

    #[test]
    fn structural_path_segments_and_order_are_significant() {
        let parent = WidgetId::explicit("parent");
        assert_ne!(
            WidgetId::derived(parent.as_u128(), &[1, 2]),
            WidgetId::derived(parent.as_u128(), &[2, 1])
        );
        assert_ne!(
            WidgetId::derived(parent.as_u128(), &[1, 2]),
            WidgetId::derived(parent.as_u128(), &[1, 2, 0])
        );
    }

    #[test]
    fn parent_identity_namespaces_structural_and_scoped_children() {
        let first = WidgetId::explicit("first-parent");
        let second = WidgetId::explicit("second-parent");
        assert_ne!(
            WidgetId::derived(first.as_u128(), &[0]),
            WidgetId::derived(second.as_u128(), &[0])
        );
        assert_ne!(
            WidgetId::scoped(first.as_u128(), "item"),
            WidgetId::scoped(second.as_u128(), "item")
        );
    }

    #[test]
    fn source_location_identity_includes_every_input() {
        let parent = WidgetId::explicit("parent");
        let base = WidgetId::scoped_location(parent.as_u128(), "src/app.rs", 10, 4);
        assert_eq!(
            base,
            WidgetId::scoped_location(parent.as_u128(), "src/app.rs", 10, 4)
        );
        assert_ne!(
            base,
            WidgetId::scoped_location(parent.as_u128(), "src/other.rs", 10, 4)
        );
        assert_ne!(
            base,
            WidgetId::scoped_location(parent.as_u128(), "src/app.rs", 11, 4)
        );
        assert_ne!(
            base,
            WidgetId::scoped_location(parent.as_u128(), "src/app.rs", 10, 5)
        );
    }
}
