//! Shared, renderer-neutral contracts for Fission 2D and 3D scenes.
//!
//! This crate intentionally has no shell, GPU, physics-engine, or game-runtime
//! dependency. Scene IR can therefore be validated, inspected, serialized,
//! and tested in a headless process.

mod asset;
mod diagnostic;
mod identity;
mod math;

pub use asset::{
    AssetBundle, AssetBundleError, AssetDescriptor, AssetHandle, AssetKind, AssetLoadState,
    ImageAsset, MaterialAsset, MeshAsset, ModelAsset, TextureAsset,
};
pub use diagnostic::{SceneDiagnostic, ScenePassStats, SceneSeverity};
pub use identity::{AssetId, NodeId, PresentationId, SceneId};
pub use math::{Bounds2, Bounds3, Quat, Rgba, Transform2, Transform3, Vec2, Vec3};

/// Wire-format version used by the first public scene alpha.
pub const SCENE_FORMAT_VERSION: u32 = 1;
