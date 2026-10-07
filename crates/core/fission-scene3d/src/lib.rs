//! Closed, retained 3D scenes for Fission.
//!
//! [`Scene3DIR`] is the serialisable application-facing authority. A
//! [`Scene3DProcessor`] validates and resolves it into a deterministic
//! [`PreparedScene3D`] consumed by renderers, picking, and headless tests.
//! Backend handles and clocks never enter either representation.

mod geometry;
mod import;
mod ir;
mod picking;
mod processing;
mod widget;

pub use import::{GltfImportError, GltfImportOptions, GltfImporter, ImportedGltf, ImportedTexture};
pub use ir::{
    AlphaMode3D, AmbientLight3D, Camera3D, CameraProjection3D, Clear3D, CullMode3D, DepthState3D,
    DirectionalLight3D, Light3D, Material3D, MaterialModel3D, Mesh3D, MeshId, MeshVertex3D,
    Model3D, ModelId, ModelNode3D, Node3D, NodeContent3D, PointLight3D, Primitive3D, ResourceId,
    Scene3DIR, Scene3DResources, Texture3D, TextureId, TextureSampling3D, Viewport3D,
};
pub use picking::{PickHit3D, Ray3D};
pub use processing::{
    DrawCommand3D, PreparedNode3D, PreparedScene3D, RenderCapabilities3D, Scene3DProcessor,
};
pub use widget::{Scene3D, Scene3DRenderPacket, SCENE3D_EMBED_MAGIC};

/// Wire-format version of the first public 3D scene alpha.
pub const SCENE3D_FORMAT_VERSION: u32 = 1;
