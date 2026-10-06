use std::collections::BTreeMap;

use fission_scene::{
    AssetBundle, AssetHandle, Bounds3, MaterialAsset, MeshAsset, ModelAsset, NodeId, Rgba, SceneId,
    TextureAsset, Transform3, Vec2, Vec3,
};
use serde::{Deserialize, Serialize};

use crate::SCENE3D_FORMAT_VERSION;

macro_rules! resource_id {
    ($name:ident) => {
        #[derive(
            Clone,
            Copy,
            Debug,
            Default,
            PartialEq,
            Eq,
            PartialOrd,
            Ord,
            Hash,
            Serialize,
            Deserialize,
        )]
        #[serde(transparent)]
        pub struct $name(pub u64);
    };
}

resource_id!(ResourceId);
resource_id!(MeshId);
resource_id!(TextureId);
resource_id!(ModelId);

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Viewport3D {
    pub origin: Vec2,
    pub size: Vec2,
}

impl Viewport3D {
    pub const fn new(width: f32, height: f32) -> Self {
        Self {
            origin: Vec2::ZERO,
            size: Vec2::new(width, height),
        }
    }

    pub fn is_valid(self) -> bool {
        self.origin.is_finite() && self.size.is_finite() && self.size.x > 0.0 && self.size.y > 0.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CameraProjection3D {
    Perspective {
        vertical_fov_radians: f32,
        near: f32,
        far: f32,
    },
    Orthographic {
        vertical_size: f32,
        near: f32,
        far: f32,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Camera3D {
    pub eye: Vec3,
    pub target: Vec3,
    pub up: Vec3,
    pub projection: CameraProjection3D,
}

impl Camera3D {
    pub fn perspective(eye: Vec3, target: Vec3, vertical_fov_radians: f32) -> Self {
        Self {
            eye,
            target,
            up: Vec3::new(0.0, 1.0, 0.0),
            projection: CameraProjection3D::Perspective {
                vertical_fov_radians,
                near: 0.1,
                far: 1_000.0,
            },
        }
    }

    pub fn orthographic(eye: Vec3, target: Vec3, vertical_size: f32) -> Self {
        Self {
            eye,
            target,
            up: Vec3::new(0.0, 1.0, 0.0),
            projection: CameraProjection3D::Orthographic {
                vertical_size,
                near: 0.1,
                far: 1_000.0,
            },
        }
    }

    pub fn is_valid(self) -> bool {
        if !self.eye.is_finite() || !self.target.is_finite() || !self.up.is_finite() {
            return false;
        }
        let view = glam::Vec3::new(
            self.target.x - self.eye.x,
            self.target.y - self.eye.y,
            self.target.z - self.eye.z,
        );
        let up = glam::Vec3::new(self.up.x, self.up.y, self.up.z);
        if view.length_squared() <= f32::EPSILON
            || up.length_squared() <= f32::EPSILON
            || view.cross(up).length_squared() <= f32::EPSILON
        {
            return false;
        }
        match self.projection {
            CameraProjection3D::Perspective {
                vertical_fov_radians,
                near,
                far,
            } => {
                vertical_fov_radians.is_finite()
                    && (0.0..std::f32::consts::PI).contains(&vertical_fov_radians)
                    && near.is_finite()
                    && far.is_finite()
                    && near > 0.0
                    && far > near
            }
            CameraProjection3D::Orthographic {
                vertical_size,
                near,
                far,
            } => {
                vertical_size.is_finite()
                    && vertical_size > 0.0
                    && near.is_finite()
                    && far.is_finite()
                    && far > near
            }
        }
    }
}

impl Default for Camera3D {
    fn default() -> Self {
        Self::perspective(Vec3::new(4.0, 3.0, 6.0), Vec3::ZERO, 45.0_f32.to_radians())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Clear3D {
    pub color: Rgba,
    pub depth: Option<f32>,
}

impl Default for Clear3D {
    fn default() -> Self {
        Self {
            color: Rgba::BLACK,
            depth: Some(1.0),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum CullMode3D {
    None,
    Front,
    #[default]
    Back,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct DepthState3D {
    pub test: bool,
    pub write: bool,
}

impl Default for DepthState3D {
    fn default() -> Self {
        Self {
            test: true,
            write: true,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum TextureSampling3D {
    Nearest,
    #[default]
    Linear,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Texture3D {
    /// Changes when decoded texture content or sampling metadata changes.
    pub revision: u64,
    pub asset: AssetHandle<TextureAsset>,
    pub sampling: TextureSampling3D,
    pub srgb: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct MeshVertex3D {
    pub position: Vec3,
    pub normal: Vec3,
    pub uv: Vec2,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Mesh3D {
    /// Changes when vertex/index content changes.
    pub revision: u64,
    pub asset: Option<AssetHandle<MeshAsset>>,
    pub vertices: Vec<MeshVertex3D>,
    pub indices: Vec<u32>,
    pub bounds: Bounds3,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AlphaMode3D {
    Opaque,
    Mask,
    Blend,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum MaterialModel3D {
    Unlit,
    MetallicRoughness { metallic: f32, roughness: f32 },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Material3D {
    /// Changes when any material property changes.
    pub revision: u64,
    pub asset: Option<AssetHandle<MaterialAsset>>,
    pub model: MaterialModel3D,
    pub base_color: Rgba,
    pub base_color_texture: Option<TextureId>,
    pub emissive_color: Rgba,
    pub emissive_texture: Option<TextureId>,
    pub alpha_mode: AlphaMode3D,
    pub alpha_cutoff: f32,
    pub double_sided: bool,
}

impl Default for Material3D {
    fn default() -> Self {
        Self {
            revision: 0,
            asset: None,
            model: MaterialModel3D::MetallicRoughness {
                metallic: 0.0,
                roughness: 0.6,
            },
            base_color: Rgba::WHITE,
            base_color_texture: None,
            emissive_color: Rgba::TRANSPARENT,
            emissive_texture: None,
            alpha_mode: AlphaMode3D::Opaque,
            alpha_cutoff: 0.5,
            double_sided: false,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ModelNode3D {
    pub parent: Option<u32>,
    pub transform: Transform3,
    pub mesh: Option<MeshId>,
    pub material: Option<ResourceId>,
    pub visible: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Model3D {
    /// Changes when the imported model graph changes.
    pub revision: u64,
    pub asset: Option<AssetHandle<ModelAsset>>,
    pub nodes: Vec<ModelNode3D>,
    pub bounds: Bounds3,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Primitive3D {
    Cube { size: Vec3 },
    Sphere { radius: f32 },
    Mesh { mesh: MeshId },
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum NodeContent3D {
    Group,
    Primitive(Primitive3D),
    Model { model: ModelId },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Node3D {
    pub id: NodeId,
    pub parent: Option<NodeId>,
    pub transform: Transform3,
    pub visible: bool,
    pub content: NodeContent3D,
    pub material: Option<ResourceId>,
    /// Stable explicit order for transparent content. Source order breaks ties.
    pub blend_order: i32,
    /// Decorative nodes remain pickable only when this is true.
    pub pickable: bool,
}

impl Node3D {
    pub fn group(id: NodeId) -> Self {
        Self {
            id,
            parent: None,
            transform: Transform3::IDENTITY,
            visible: true,
            content: NodeContent3D::Group,
            material: None,
            blend_order: 0,
            pickable: false,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct AmbientLight3D {
    pub color: Rgba,
    pub intensity: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct DirectionalLight3D {
    /// Direction travelled by light rays in world space.
    pub direction: Vec3,
    pub color: Rgba,
    pub intensity: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct PointLight3D {
    pub position: Vec3,
    pub color: Rgba,
    pub intensity: f32,
    pub range: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Light3D {
    Ambient(AmbientLight3D),
    Directional(DirectionalLight3D),
    Point(PointLight3D),
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Scene3DResources {
    pub meshes: BTreeMap<MeshId, Mesh3D>,
    pub textures: BTreeMap<TextureId, Texture3D>,
    pub materials: BTreeMap<ResourceId, Material3D>,
    pub models: BTreeMap<ModelId, Model3D>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Scene3DIR {
    pub format_version: u32,
    pub id: SceneId,
    /// Deterministic metadata used to resolve typed texture handles to cooked
    /// package sources. Encoded media remains outside the serialisable scene.
    pub assets: AssetBundle,
    pub viewport: Viewport3D,
    pub camera: Camera3D,
    pub clear: Clear3D,
    pub depth: DepthState3D,
    pub cull_mode: CullMode3D,
    pub resources: Scene3DResources,
    pub lights: Vec<Light3D>,
    /// Source order is authoritative where no explicit order differs.
    pub nodes: Vec<Node3D>,
}

impl Scene3DIR {
    pub fn new(id: SceneId, viewport: Viewport3D) -> Self {
        Self {
            format_version: SCENE3D_FORMAT_VERSION,
            id,
            assets: AssetBundle {
                format_version: AssetBundle::FORMAT_VERSION,
                id: format!("scene3d-{}", id.get()),
                assets: Vec::new(),
            },
            viewport,
            camera: Camera3D::default(),
            clear: Clear3D::default(),
            depth: DepthState3D::default(),
            cull_mode: CullMode3D::Back,
            resources: Scene3DResources::default(),
            lights: Vec::new(),
            nodes: Vec::new(),
        }
    }
}
