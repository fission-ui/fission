use std::collections::BTreeMap;

use fission_scene::{
    AssetBundle, AssetHandle, AssetId, Bounds2, ImageAsset, NodeId, Rgba, SceneId, Transform2, Vec2,
};
use serde::{Deserialize, Serialize};

use crate::{Interaction2D, Rect2D, SCENE2D_FORMAT_VERSION};

#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
#[serde(transparent)]
pub struct ResourceId(pub u64);

/// Typed image handle used by 2D scenes.
#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
#[serde(transparent)]
pub struct ImageHandle2D(pub AssetId);

impl ImageHandle2D {
    pub const fn new(id: AssetId) -> Self {
        Self(id)
    }

    pub const fn id(self) -> AssetId {
        self.0
    }
}

impl From<AssetHandle<ImageAsset>> for ImageHandle2D {
    fn from(handle: AssetHandle<ImageAsset>) -> Self {
        Self(handle.id())
    }
}

impl From<ImageHandle2D> for AssetHandle<ImageAsset> {
    fn from(handle: ImageHandle2D) -> Self {
        Self::new(handle.0)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum BlendMode2D {
    #[default]
    Normal,
    Multiply,
    Screen,
    Add,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Viewport2D {
    /// Widget-local logical-pixel rectangle occupied by the scene.
    pub rect: Rect2D,
}

impl Viewport2D {
    pub const fn new(width: f32, height: f32) -> Self {
        Self {
            rect: Rect2D::new(Vec2::ZERO, Vec2::new(width, height)),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Camera2D {
    pub center: Vec2,
    pub rotation_radians: f32,
    /// Logical viewport pixels per world unit.
    pub zoom: f32,
}

impl Default for Camera2D {
    fn default() -> Self {
        Self {
            center: Vec2::ZERO,
            rotation_radians: 0.0,
            zoom: 1.0,
        }
    }
}

impl Camera2D {
    pub fn is_valid(self) -> bool {
        self.center.is_finite()
            && self.rotation_radians.is_finite()
            && self.zoom.is_finite()
            && self.zoom > 0.0
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Clip2D {
    Rectangle { rect: Rect2D },
    Path { path: ResourceId },
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Fill2D {
    pub color: Rgba,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Stroke2D {
    pub color: Rgba,
    pub width: f32,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct PathStyle2D {
    pub fill: Option<Fill2D>,
    pub stroke: Option<Stroke2D>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum ImageSampling2D {
    Nearest,
    #[default]
    Linear,
}

/// Pixel-space source rectangle within a decoded image.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct ImageSource2D {
    pub rect: Rect2D,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PathCommand2D {
    MoveTo {
        point: Vec2,
    },
    LineTo {
        point: Vec2,
    },
    QuadraticTo {
        control: Vec2,
        point: Vec2,
    },
    CubicTo {
        control1: Vec2,
        control2: Vec2,
        point: Vec2,
    },
    Close,
}

/// A retained vector path. Bounds are generated with the path and validated so
/// culling and picking never need renderer-specific path objects.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PathResource2D {
    pub commands: Vec<PathCommand2D>,
    pub bounds: Bounds2,
}

/// One already-shaped glyph from Fission's resolved text pipeline.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Glyph2D {
    pub id: u32,
    pub position: Vec2,
    pub advance: f32,
}

/// Retained shaped text. `font_key` identifies the resolved font face rather
/// than asking each renderer to choose or shape a font independently.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TextResource2D {
    pub text: String,
    pub font_key: String,
    pub font_size: f32,
    pub glyphs: Vec<Glyph2D>,
    pub bounds: Bounds2,
    pub color: Rgba,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct SpriteFrame2D {
    pub source: ImageSource2D,
    pub size: Vec2,
    pub pivot: Vec2,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SpriteSheet2D {
    pub image: ImageHandle2D,
    pub frames: Vec<SpriteFrame2D>,
    pub sampling: ImageSampling2D,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Sprite2D {
    pub sheet: ResourceId,
    pub frame: u32,
    pub tint: Rgba,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct ImageInstance2D {
    pub transform: Transform2,
    pub destination: Rect2D,
    pub source: Option<ImageSource2D>,
    pub tint: Rgba,
    pub opacity: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ImageBatch2D {
    pub image: ImageHandle2D,
    pub sampling: ImageSampling2D,
    pub instances: Vec<ImageInstance2D>,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct SpriteInstance2D {
    pub transform: Transform2,
    pub frame: u32,
    pub tint: Rgba,
    pub opacity: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SpriteBatch2D {
    pub sheet: ResourceId,
    pub instances: Vec<SpriteInstance2D>,
}

/// Closed visual vocabulary for the first 2D alpha.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum NodeContent2D {
    Group,
    Rectangle {
        rect: Rect2D,
        style: PathStyle2D,
        corner_radius: f32,
    },
    Path {
        path: ResourceId,
        style: PathStyle2D,
    },
    Text {
        text: ResourceId,
    },
    Image {
        image: ImageHandle2D,
        destination: Rect2D,
        source: Option<ImageSource2D>,
        sampling: ImageSampling2D,
        tint: Rgba,
    },
    Sprite(Sprite2D),
    ImageBatch(ImageBatch2D),
    SpriteBatch(SpriteBatch2D),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Node2D {
    pub id: NodeId,
    pub parent: Option<NodeId>,
    pub transform: Transform2,
    pub visible: bool,
    pub opacity: f32,
    pub layer: i32,
    pub blend_mode: BlendMode2D,
    pub clip: Option<Clip2D>,
    pub content: NodeContent2D,
    /// `None` is intentionally decorative and pointer-transparent.
    pub interaction: Option<Interaction2D>,
}

impl Node2D {
    pub fn group(id: NodeId) -> Self {
        Self {
            id,
            parent: None,
            transform: Transform2::IDENTITY,
            visible: true,
            opacity: 1.0,
            layer: 0,
            blend_mode: BlendMode2D::Normal,
            clip: None,
            content: NodeContent2D::Group,
            interaction: None,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Scene2DResources {
    pub paths: BTreeMap<ResourceId, PathResource2D>,
    pub text: BTreeMap<ResourceId, TextResource2D>,
    pub sprite_sheets: BTreeMap<ResourceId, SpriteSheet2D>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Scene2DIR {
    pub format_version: u32,
    pub id: SceneId,
    /// Deterministic metadata used to resolve every typed image handle to its
    /// packaged source and content digest.
    pub assets: AssetBundle,
    pub viewport: Viewport2D,
    pub camera: Camera2D,
    pub resources: Scene2DResources,
    /// Source order is authoritative for equal layers.
    pub nodes: Vec<Node2D>,
}

impl Scene2DIR {
    pub fn new(id: SceneId, viewport: Viewport2D) -> Self {
        Self {
            format_version: SCENE2D_FORMAT_VERSION,
            id,
            assets: AssetBundle {
                format_version: AssetBundle::FORMAT_VERSION,
                id: format!("scene2d-{}", id.get()),
                assets: Vec::new(),
            },
            viewport,
            camera: Camera2D::default(),
            resources: Scene2DResources::default(),
            nodes: Vec::new(),
        }
    }
}
