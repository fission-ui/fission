//! Closed, renderer-independent 2D scene commands.

use std::cmp::Ordering;

use fission_ir::op::{Color, ImageRequest, ImageSource};
use fission_ir::WidgetId;
use serde::{Deserialize, Serialize};

use crate::{Bounds2D, Degrees, Place, Px, Size, StableKey, StableKeyValue, Tick};

/// Stable identity for one expert scene declaration.
///
/// It is constructed from a typed domain key rather than an authored string or
/// a process-random hash.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SceneNodeId(pub StableKeyValue);

impl SceneNodeId {
    pub fn from_key(key: &impl StableKey) -> Self {
        Self(key.stable_key())
    }

    /// Converts the structural game identity into Fission's retained widget
    /// identity domain for scene hit testing and runtime-state preservation.
    pub fn widget_id(&self) -> WidgetId {
        WidgetId::explicit(&format!("fission.game.scene:{}", self.0.canonical()))
    }
}

/// Painter's-order layer. Lower values render first.
#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
#[serde(transparent)]
pub struct Layer(pub i32);

/// Which local point is placed at a transform's translation.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Anchor {
    #[default]
    Center,
    TopLeft,
}

/// Two-dimensional presentation transform.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Transform2D {
    pub translation: Place,
    pub rotation: Degrees,
    pub scale_x: f32,
    pub scale_y: f32,
    pub anchor: Anchor,
}

impl Default for Transform2D {
    fn default() -> Self {
        Self {
            translation: Place::default(),
            rotation: Degrees::default(),
            scale_x: 1.0,
            scale_y: 1.0,
            anchor: Anchor::Center,
        }
    }
}

impl Transform2D {
    pub fn at(translation: Place) -> Self {
        Self {
            translation,
            ..Self::default()
        }
    }

    pub fn is_finite(self) -> bool {
        self.translation.x.0.is_finite()
            && self.translation.y.0.is_finite()
            && self.rotation.0.is_finite()
            && self.scale_x.is_finite()
            && self.scale_y.is_finite()
    }
}

/// Typed reference to an image prepared by an asset pipeline.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ImageAsset {
    pub id: StableKeyValue,
    /// Renderer-facing source and loading policy produced by the asset build.
    pub request: ImageRequest,
    pub pixel_width: u32,
    pub pixel_height: u32,
}

impl ImageAsset {
    pub fn new(
        key: &impl StableKey,
        request: ImageRequest,
        pixel_width: u32,
        pixel_height: u32,
    ) -> Self {
        Self {
            id: key.stable_key(),
            request,
            pixel_width,
            pixel_height,
        }
    }

    /// Creates a compiled application asset reference.
    pub fn asset(
        key: &impl StableKey,
        path: impl Into<String>,
        pixel_width: u32,
        pixel_height: u32,
    ) -> Self {
        Self::new(
            key,
            ImageRequest {
                source: ImageSource::Asset { path: path.into() },
                ..Default::default()
            },
            pixel_width,
            pixel_height,
        )
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ImageSampling {
    Nearest,
    #[default]
    Linear,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum BlendMode2D {
    #[default]
    SourceOver,
    Multiply,
    Screen,
    Add,
}

/// One validated sprite instance.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ImageInstance2D {
    pub id: SceneNodeId,
    pub transform: Transform2D,
    pub size: Size,
    pub opacity: f32,
}

/// Closed command set consumed by 2D render backends.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "command")]
pub enum Scene2DCommand {
    Clear {
        color: Color,
    },
    DrawRect {
        id: SceneNodeId,
        bounds: Bounds2D,
        fill: Color,
        layer: Layer,
        opacity: f32,
    },
    DrawImage {
        id: SceneNodeId,
        image: ImageAsset,
        transform: Transform2D,
        size: Size,
        layer: Layer,
        opacity: f32,
        sampling: ImageSampling,
        blend_mode: BlendMode2D,
    },
    DrawText {
        id: SceneNodeId,
        text: String,
        transform: Transform2D,
        size: Px,
        color: Color,
        layer: Layer,
        opacity: f32,
    },
    ImageBatch {
        image: ImageAsset,
        layer: Layer,
        sampling: ImageSampling,
        blend_mode: BlendMode2D,
        instances: Vec<ImageInstance2D>,
    },
}

impl Scene2DCommand {
    fn layer(&self) -> Layer {
        match self {
            Self::Clear { .. } => Layer(i32::MIN),
            Self::DrawRect { layer, .. }
            | Self::DrawImage { layer, .. }
            | Self::DrawText { layer, .. }
            | Self::ImageBatch { layer, .. } => *layer,
        }
    }

    fn id(&self) -> Option<&SceneNodeId> {
        match self {
            Self::DrawRect { id, .. } | Self::DrawImage { id, .. } | Self::DrawText { id, .. } => {
                Some(id)
            }
            Self::Clear { .. } | Self::ImageBatch { .. } => None,
        }
    }

    fn bounds(&self) -> Option<Bounds2D> {
        match self {
            Self::DrawRect { bounds, .. } => Some(*bounds),
            Self::DrawImage {
                transform, size, ..
            } => Some(transformed_bounds(*transform, *size)),
            Self::DrawText { .. } | Self::Clear { .. } | Self::ImageBatch { .. } => None,
        }
    }
}

/// A validation or optimization diagnostic tied to a scene declaration.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "kind")]
pub enum GameDiagnostic {
    DuplicateNodeId { id: SceneNodeId },
    InvalidTransform { id: SceneNodeId, reason: String },
    InvalidSize { id: SceneNodeId, size: Size },
    InvalidOpacity { id: SceneNodeId, opacity: f32 },
    MissingAssetDimensions { id: SceneNodeId, asset: ImageAsset },
    SkippedInvalidDeclaration { id: SceneNodeId, reason: String },
}

/// Validated scene output for one simulation state.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Scene2DIR {
    pub tick: Tick,
    pub commands: Vec<Scene2DCommand>,
    pub diagnostics: Vec<GameDiagnostic>,
}

#[derive(Clone, Debug)]
struct Declaration {
    source_order: usize,
    command: Scene2DCommand,
}

/// Expert retained-scene builder.
///
/// `Scene2D` records structured declarations. [`finish`](Self::finish)
/// validates them, applies stable ordering and visibility culling, and batches
/// only adjacent compatible images without reordering transparent content.
#[derive(Clone, Debug, Default)]
pub struct Scene2D {
    camera_bounds: Option<Bounds2D>,
    declarations: Vec<Declaration>,
}

impl Scene2D {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn camera_bounds(&mut self, bounds: Bounds2D) -> &mut Self {
        self.camera_bounds = Some(bounds);
        self
    }

    pub fn clear(&mut self, color: Color) -> &mut Self {
        self.push(Scene2DCommand::Clear { color });
        self
    }

    pub fn rect(
        &mut self,
        id: SceneNodeId,
        bounds: Bounds2D,
        fill: Color,
        layer: Layer,
    ) -> &mut Self {
        self.push(Scene2DCommand::DrawRect {
            id,
            bounds,
            fill,
            layer,
            opacity: 1.0,
        });
        self
    }

    pub fn image(
        &mut self,
        id: SceneNodeId,
        image: ImageAsset,
        transform: Transform2D,
        size: Size,
        layer: Layer,
    ) -> &mut Self {
        self.push(Scene2DCommand::DrawImage {
            id,
            image,
            transform,
            size,
            layer,
            opacity: 1.0,
            sampling: ImageSampling::Linear,
            blend_mode: BlendMode2D::SourceOver,
        });
        self
    }

    pub fn text(
        &mut self,
        id: SceneNodeId,
        text: impl Into<String>,
        transform: Transform2D,
        size: Px,
        color: Color,
        layer: Layer,
    ) -> &mut Self {
        self.push(Scene2DCommand::DrawText {
            id,
            text: text.into(),
            transform,
            size,
            color,
            layer,
            opacity: 1.0,
        });
        self
    }

    pub fn command(&mut self, command: Scene2DCommand) -> &mut Self {
        self.push(command);
        self
    }

    fn push(&mut self, command: Scene2DCommand) {
        self.declarations.push(Declaration {
            source_order: self.declarations.len(),
            command,
        });
    }

    pub fn finish(self, tick: Tick) -> Scene2DIR {
        let mut diagnostics = Vec::new();
        let mut seen = std::collections::BTreeSet::new();
        let mut declarations = Vec::new();

        for declaration in self.declarations {
            let Some(validated) = validate(declaration, &mut seen, &mut diagnostics) else {
                continue;
            };
            if self.camera_bounds.is_some_and(|camera| {
                validated
                    .command
                    .bounds()
                    .is_some_and(|bounds| !camera.overlaps(bounds))
            }) {
                continue;
            }
            declarations.push(validated);
        }

        declarations.sort_by(|left, right| {
            left.command
                .layer()
                .cmp(&right.command.layer())
                .then(left.source_order.cmp(&right.source_order))
                .then_with(|| match (left.command.id(), right.command.id()) {
                    (Some(left), Some(right)) => left.cmp(right),
                    _ => Ordering::Equal,
                })
        });

        Scene2DIR {
            tick,
            commands: batch_adjacent_images(declarations),
            diagnostics,
        }
    }
}

fn validate(
    declaration: Declaration,
    seen: &mut std::collections::BTreeSet<SceneNodeId>,
    diagnostics: &mut Vec<GameDiagnostic>,
) -> Option<Declaration> {
    let Some(id) = declaration.command.id().cloned() else {
        return Some(declaration);
    };
    if !seen.insert(id.clone()) {
        diagnostics.push(GameDiagnostic::DuplicateNodeId { id });
        return None;
    }

    let error = match &declaration.command {
        Scene2DCommand::DrawRect {
            bounds, opacity, ..
        } => (!bounds.is_valid())
            .then(|| GameDiagnostic::InvalidSize {
                id: id.clone(),
                size: Size::new(bounds.width(), bounds.height()),
            })
            .or_else(|| invalid_opacity(&id, *opacity)),
        Scene2DCommand::DrawImage {
            image,
            transform,
            size,
            opacity,
            ..
        } => (!transform.is_finite())
            .then(|| GameDiagnostic::InvalidTransform {
                id: id.clone(),
                reason: "transform contains a non-finite value".into(),
            })
            .or_else(|| {
                (!size.is_valid()).then(|| GameDiagnostic::InvalidSize {
                    id: id.clone(),
                    size: *size,
                })
            })
            .or_else(|| invalid_opacity(&id, *opacity))
            .or_else(|| {
                (image.pixel_width == 0 || image.pixel_height == 0).then(|| {
                    GameDiagnostic::MissingAssetDimensions {
                        id: id.clone(),
                        asset: image.clone(),
                    }
                })
            }),
        Scene2DCommand::DrawText {
            transform,
            size,
            opacity,
            ..
        } => (!transform.is_finite())
            .then(|| GameDiagnostic::InvalidTransform {
                id: id.clone(),
                reason: "transform contains a non-finite value".into(),
            })
            .or_else(|| {
                (!size.0.is_finite() || size.0 < 0.0).then(|| GameDiagnostic::InvalidSize {
                    id: id.clone(),
                    size: Size::new(*size, Px::ZERO),
                })
            })
            .or_else(|| invalid_opacity(&id, *opacity)),
        Scene2DCommand::Clear { .. } | Scene2DCommand::ImageBatch { .. } => None,
    };

    if let Some(error) = error {
        diagnostics.push(error);
        diagnostics.push(GameDiagnostic::SkippedInvalidDeclaration {
            id,
            reason: "scene declaration failed validation".into(),
        });
        None
    } else {
        Some(declaration)
    }
}

fn invalid_opacity(id: &SceneNodeId, opacity: f32) -> Option<GameDiagnostic> {
    (!(0.0..=1.0).contains(&opacity) || !opacity.is_finite()).then(|| {
        GameDiagnostic::InvalidOpacity {
            id: id.clone(),
            opacity,
        }
    })
}

fn transformed_bounds(transform: Transform2D, size: Size) -> Bounds2D {
    let scaled = Size::new(
        Px(size.width.0 * transform.scale_x.abs()),
        Px(size.height.0 * transform.scale_y.abs()),
    );
    match transform.anchor {
        Anchor::Center => Bounds2D::from_center(transform.translation, scaled),
        Anchor::TopLeft => Bounds2D::from_top_left(transform.translation, scaled),
    }
}

fn batch_adjacent_images(declarations: Vec<Declaration>) -> Vec<Scene2DCommand> {
    let mut commands = Vec::new();
    for declaration in declarations {
        match declaration.command {
            Scene2DCommand::DrawImage {
                id,
                image,
                transform,
                size,
                layer,
                opacity,
                sampling,
                blend_mode,
            } => {
                let instance = ImageInstance2D {
                    id,
                    transform,
                    size,
                    opacity,
                };
                match commands.last_mut() {
                    Some(Scene2DCommand::ImageBatch {
                        image: existing_image,
                        layer: existing_layer,
                        sampling: existing_sampling,
                        blend_mode: existing_blend_mode,
                        instances,
                    }) if *existing_image == image
                        && *existing_layer == layer
                        && *existing_sampling == sampling
                        && *existing_blend_mode == blend_mode =>
                    {
                        instances.push(instance);
                    }
                    _ => commands.push(Scene2DCommand::ImageBatch {
                        image,
                        layer,
                        sampling,
                        blend_mode,
                        instances: vec![instance],
                    }),
                }
            }
            command => commands.push(command),
        }
    }
    commands
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node(value: u32) -> SceneNodeId {
        SceneNodeId::from_key(&value)
    }

    #[test]
    fn finish_validates_culls_and_batches_without_reordering() {
        let image = ImageAsset::asset(&7_u32, "sprite.png", 64, 64);
        let mut scene = Scene2D::new();
        scene.camera_bounds(Bounds2D::from_top_left(
            Place::new(Px(0.0), Px(0.0)),
            Size::new(Px(100.0), Px(100.0)),
        ));
        scene.image(
            node(1),
            image.clone(),
            Transform2D::at(Place::new(Px(10.0), Px(10.0))),
            Size::new(Px(8.0), Px(8.0)),
            Layer(2),
        );
        scene.image(
            node(2),
            image,
            Transform2D::at(Place::new(Px(20.0), Px(20.0))),
            Size::new(Px(8.0), Px(8.0)),
            Layer(2),
        );
        scene.rect(
            node(3),
            Bounds2D::from_top_left(
                Place::new(Px(200.0), Px(200.0)),
                Size::new(Px(5.0), Px(5.0)),
            ),
            Color::WHITE,
            Layer(1),
        );

        let ir = scene.finish(Tick(4));
        assert!(ir.diagnostics.is_empty());
        assert_eq!(ir.commands.len(), 1);
        assert!(matches!(
            &ir.commands[0],
            Scene2DCommand::ImageBatch { instances, .. } if instances.len() == 2
        ));
    }

    #[test]
    fn duplicate_and_invalid_nodes_are_reported_and_skipped() {
        let mut scene = Scene2D::new();
        scene.rect(
            node(1),
            Bounds2D::from_top_left(Place::new(Px(0.0), Px(0.0)), Size::new(Px(10.0), Px(10.0))),
            Color::WHITE,
            Layer(0),
        );
        scene.rect(
            node(1),
            Bounds2D::from_top_left(Place::new(Px(5.0), Px(5.0)), Size::new(Px(2.0), Px(2.0))),
            Color::BLACK,
            Layer(0),
        );

        let ir = scene.finish(Tick(0));
        assert_eq!(ir.commands.len(), 1);
        assert!(matches!(
            ir.diagnostics.as_slice(),
            [GameDiagnostic::DuplicateNodeId { .. }]
        ));
    }
}
