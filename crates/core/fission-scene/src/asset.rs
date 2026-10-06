//! Typed asset handles and deterministic bundle metadata.

use std::collections::BTreeSet;
use std::marker::PhantomData;

use serde::{Deserialize, Serialize};

use crate::AssetId;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AssetKind {
    Image,
    Texture,
    Mesh,
    Material,
    Model,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssetDescriptor {
    pub id: AssetId,
    pub kind: AssetKind,
    pub source: String,
    /// Lowercase SHA-256 of the packaged source bytes.
    pub sha256: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssetBundle {
    pub format_version: u32,
    pub id: String,
    pub assets: Vec<AssetDescriptor>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AssetBundleError {
    pub asset: Option<AssetId>,
    pub message: String,
}

impl AssetBundle {
    pub const FORMAT_VERSION: u32 = 1;

    pub fn validate(&self) -> Result<(), Vec<AssetBundleError>> {
        let mut errors = Vec::new();
        if self.format_version != Self::FORMAT_VERSION {
            errors.push(AssetBundleError {
                asset: None,
                message: format!(
                    "unsupported asset bundle version {}; expected {}",
                    self.format_version,
                    Self::FORMAT_VERSION
                ),
            });
        }
        if self.id.trim().is_empty() {
            errors.push(AssetBundleError {
                asset: None,
                message: "asset bundle id is empty".into(),
            });
        }

        let mut ids = BTreeSet::new();
        for asset in &self.assets {
            if !ids.insert(asset.id) {
                errors.push(AssetBundleError {
                    asset: Some(asset.id),
                    message: "asset id is duplicated".into(),
                });
            }
            if asset.source.trim().is_empty() {
                errors.push(AssetBundleError {
                    asset: Some(asset.id),
                    message: "asset source is empty".into(),
                });
            }
            if !is_sha256(&asset.sha256) {
                errors.push(AssetBundleError {
                    asset: Some(asset.id),
                    message: "asset sha256 must be 64 lowercase hexadecimal digits".into(),
                });
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
}

fn is_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AssetLoadState {
    Loading,
    Ready,
    Failed { message: String },
}

#[derive(Serialize, Deserialize)]
#[serde(transparent)]
pub struct AssetHandle<T> {
    id: AssetId,
    #[serde(skip)]
    marker: PhantomData<fn() -> T>,
}

impl<T> AssetHandle<T> {
    pub const fn new(id: AssetId) -> Self {
        Self {
            id,
            marker: PhantomData,
        }
    }

    pub const fn id(&self) -> AssetId {
        self.id
    }
}

impl<T> Copy for AssetHandle<T> {}

impl<T> Clone for AssetHandle<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> std::fmt::Debug for AssetHandle<T> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_tuple("AssetHandle")
            .field(&self.id)
            .finish()
    }
}

impl<T> PartialEq for AssetHandle<T> {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl<T> Eq for AssetHandle<T> {}

impl<T> PartialOrd for AssetHandle<T> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl<T> Ord for AssetHandle<T> {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.id.cmp(&other.id)
    }
}

impl<T> std::hash::Hash for AssetHandle<T> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        std::hash::Hash::hash(&self.id, state);
    }
}

pub enum ImageAsset {}
pub enum TextureAsset {}
pub enum MeshAsset {}
pub enum MaterialAsset {}
pub enum ModelAsset {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundle_validation_accumulates_actionable_failures() {
        let bundle = AssetBundle {
            format_version: 99,
            id: String::new(),
            assets: vec![AssetDescriptor {
                id: AssetId(4),
                kind: AssetKind::Image,
                source: String::new(),
                sha256: "bad".into(),
            }],
        };

        assert_eq!(bundle.validate().unwrap_err().len(), 4);
    }

    #[test]
    fn handles_are_typed_but_keep_the_portable_id() {
        let handle = AssetHandle::<TextureAsset>::new(AssetId(7));
        assert_eq!(handle.id(), AssetId(7));
        assert_eq!(serde_json::to_value(handle).unwrap(), 7);
    }

    #[test]
    fn application_marker_types_need_no_trait_implementations() {
        enum ApplicationAsset {}

        let first = AssetHandle::<ApplicationAsset>::new(AssetId(1));
        let same = AssetHandle::<ApplicationAsset>::new(AssetId(1));
        assert_eq!(first, same);
        assert_eq!(format!("{first:?}"), "AssetHandle(AssetId(1))");
        assert_eq!(std::collections::BTreeSet::from([first]).len(), 1);
    }
}
