use std::collections::{BTreeMap, BTreeSet};

use fission_scene::{AssetKind, Bounds2, NodeId, Rgba, SceneDiagnostic, ScenePassStats, Vec2};
use serde::{Deserialize, Serialize};

use crate::{
    Affine2, BlendMode2D, Clip2D, ImageBatch2D, ImageHandle2D, ImageInstance2D, ImageSampling2D,
    ImageSource2D, Interaction2D, NodeContent2D, PathResource2D, PathStyle2D, Rect2D, ResourceId,
    Scene2DIR, SpriteFrame2D, Stroke2D, SCENE2D_FORMAT_VERSION,
};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PreparedClip2D {
    pub shape: Clip2D,
    pub view_transform: Affine2,
    pub view_bounds: Bounds2,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DrawMetadata2D {
    pub node: NodeId,
    pub source_index: u32,
    pub layer: i32,
    pub blend_mode: BlendMode2D,
    pub opacity: f32,
    pub scene_transform: Affine2,
    pub view_transform: Affine2,
    pub local_bounds: Bounds2,
    pub scene_bounds: Bounds2,
    pub view_bounds: Bounds2,
    pub clips: Vec<PreparedClip2D>,
    pub interaction: Option<Interaction2D>,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct PreparedImageInstance2D {
    pub view_transform: Affine2,
    pub destination: Rect2D,
    pub source: Option<ImageSource2D>,
    pub tint: Rgba,
    pub opacity: f32,
    pub view_bounds: Bounds2,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct PreparedSpriteInstance2D {
    pub view_transform: Affine2,
    pub destination: Rect2D,
    pub source: ImageSource2D,
    pub tint: Rgba,
    pub opacity: f32,
    pub view_bounds: Bounds2,
}

/// Renderer input. Batch variants deliberately contain all visible instances
/// in one command so adapters cannot accidentally turn them into retained
/// child widgets.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DrawCommand2D {
    Rectangle {
        metadata: DrawMetadata2D,
        rect: Rect2D,
        style: PathStyle2D,
        corner_radius: f32,
    },
    Path {
        metadata: DrawMetadata2D,
        path: ResourceId,
        style: PathStyle2D,
    },
    Text {
        metadata: DrawMetadata2D,
        text: ResourceId,
    },
    Image {
        metadata: DrawMetadata2D,
        image: ImageHandle2D,
        destination: Rect2D,
        source: Option<ImageSource2D>,
        sampling: ImageSampling2D,
        tint: Rgba,
    },
    ImageBatch {
        metadata: DrawMetadata2D,
        image: ImageHandle2D,
        sampling: ImageSampling2D,
        instances: Vec<PreparedImageInstance2D>,
    },
    SpriteBatch {
        metadata: DrawMetadata2D,
        image: ImageHandle2D,
        sampling: ImageSampling2D,
        instances: Vec<PreparedSpriteInstance2D>,
    },
}

impl DrawCommand2D {
    pub fn metadata(&self) -> &DrawMetadata2D {
        match self {
            Self::Rectangle { metadata, .. }
            | Self::Path { metadata, .. }
            | Self::Text { metadata, .. }
            | Self::Image { metadata, .. }
            | Self::ImageBatch { metadata, .. }
            | Self::SpriteBatch { metadata, .. } => metadata,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PickHit2D {
    pub node: NodeId,
    pub instance: Option<u32>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PreparedScene2D {
    pub scene_to_view: Affine2,
    pub view_to_scene: Affine2,
    pub draws: Vec<DrawCommand2D>,
    pub diagnostics: Vec<SceneDiagnostic>,
    pub stats: ScenePassStats,
}

impl PreparedScene2D {
    /// Pick topmost visible content at a widget-local viewport position.
    pub fn pick(&self, view_position: Vec2) -> Option<PickHit2D> {
        self.pick_impl(view_position, false)
    }

    /// Pick only nodes that explicitly declare interaction. Decorative scene
    /// content is therefore pointer-transparent by construction.
    pub fn pick_interactive(&self, view_position: Vec2) -> Option<PickHit2D> {
        self.pick_impl(view_position, true)
    }

    pub fn bounds(&self, node: NodeId) -> Option<Bounds2> {
        self.draws
            .iter()
            .find(|draw| draw.metadata().node == node)
            .map(|draw| draw.metadata().scene_bounds)
    }

    fn pick_impl(&self, view_position: Vec2, interactive_only: bool) -> Option<PickHit2D> {
        for draw in self.draws.iter().rev() {
            let metadata = draw.metadata();
            if interactive_only
                && !metadata
                    .interaction
                    .as_ref()
                    .is_some_and(Interaction2D::is_interactive)
            {
                continue;
            }
            if !metadata.view_bounds.contains(view_position)
                || !metadata
                    .clips
                    .iter()
                    .all(|clip| clip.view_bounds.contains(view_position))
            {
                continue;
            }
            match draw {
                DrawCommand2D::ImageBatch { instances, .. } => {
                    if let Some((index, _)) = instances
                        .iter()
                        .enumerate()
                        .rev()
                        .find(|(_, instance)| instance.view_bounds.contains(view_position))
                    {
                        return Some(PickHit2D {
                            node: metadata.node,
                            instance: Some(index as u32),
                        });
                    }
                }
                DrawCommand2D::SpriteBatch { instances, .. } => {
                    if let Some((index, _)) = instances
                        .iter()
                        .enumerate()
                        .rev()
                        .find(|(_, instance)| instance.view_bounds.contains(view_position))
                    {
                        return Some(PickHit2D {
                            node: metadata.node,
                            instance: Some(index as u32),
                        });
                    }
                }
                _ => {
                    let local = metadata
                        .view_transform
                        .inverse()?
                        .transform_point(view_position);
                    if metadata.local_bounds.contains(local) {
                        return Some(PickHit2D {
                            node: metadata.node,
                            instance: None,
                        });
                    }
                }
            }
        }
        None
    }
}

#[derive(Clone, Debug)]
struct WorldNode {
    scene_transform: Affine2,
    visible: bool,
    opacity: f32,
    clips: Vec<(Clip2D, Affine2, Bounds2)>,
}

/// Deterministic validation, hierarchy, culling, and render-plan pass.
pub struct Scene2DProcessor;

impl Scene2DProcessor {
    pub fn prepare(scene: &Scene2DIR) -> PreparedScene2D {
        let mut diagnostics = Vec::new();
        let mut stats = ScenePassStats::default();
        if scene.format_version != SCENE2D_FORMAT_VERSION {
            diagnostics.push(SceneDiagnostic::error(
                "scene2d.unsupported-format",
                format!(
                    "unsupported 2D scene format {}; expected {}",
                    scene.format_version, SCENE2D_FORMAT_VERSION
                ),
            ));
        }
        if !scene.viewport.rect.is_valid()
            || scene.viewport.rect.size.x <= 0.0
            || scene.viewport.rect.size.y <= 0.0
        {
            diagnostics.push(SceneDiagnostic::error(
                "scene2d.invalid-viewport",
                "viewport must have a finite positive size",
            ));
        }
        if !scene.camera.is_valid() {
            diagnostics.push(SceneDiagnostic::error(
                "scene2d.invalid-camera",
                "camera center and rotation must be finite and zoom must be positive",
            ));
        }

        let scene_to_view = camera_transform(scene);
        let view_to_scene = scene_to_view.inverse().unwrap_or(Affine2::IDENTITY);
        let view_bounds = scene.viewport.rect.bounds();
        let scene_view_bounds = view_to_scene.transform_bounds(view_bounds);

        validate_resources(scene, &mut diagnostics);
        if let Err(errors) = scene.assets.validate() {
            diagnostics.extend(errors.into_iter().map(|error| {
                let diagnostic = SceneDiagnostic::error(
                    "scene2d.invalid-asset-bundle",
                    format!("asset bundle '{}': {}", scene.assets.id, error.message),
                );
                error
                    .asset
                    .map_or(diagnostic.clone(), |asset| diagnostic.for_asset(asset))
            }));
        }
        let mut by_id = BTreeMap::new();
        let mut duplicate_ids = BTreeSet::new();
        for (index, node) in scene.nodes.iter().enumerate() {
            if by_id.insert(node.id, index).is_some() {
                duplicate_ids.insert(node.id);
                diagnostics.push(
                    SceneDiagnostic::error(
                        "scene2d.duplicate-node",
                        format!("node id {} is duplicated", node.id.get()),
                    )
                    .for_node(node.id),
                );
            }
        }

        let mut visit = vec![0_u8; scene.nodes.len()];
        let mut world = vec![None; scene.nodes.len()];
        for index in 0..scene.nodes.len() {
            resolve_world(
                scene,
                index,
                &by_id,
                &duplicate_ids,
                &mut visit,
                &mut world,
                &mut diagnostics,
            );
        }

        let mut draws = Vec::new();
        let mut retained = BTreeSet::new();
        for (index, node) in scene.nodes.iter().enumerate() {
            stats.considered = stats.considered.saturating_add(1);
            if duplicate_ids.contains(&node.id) {
                stats.rejected = stats.rejected.saturating_add(1);
                continue;
            }
            let Some(world_node) = world[index].as_ref() else {
                stats.rejected = stats.rejected.saturating_add(1);
                continue;
            };
            if !world_node.visible || world_node.opacity <= 0.0 {
                stats.culled = stats.culled.saturating_add(1);
                continue;
            }
            match prepare_node(
                scene,
                node,
                index,
                world_node,
                scene_to_view,
                scene_view_bounds,
                view_bounds,
                &mut diagnostics,
                &mut retained,
            ) {
                NodePreparation::Draw(draw) => {
                    if matches!(
                        draw,
                        DrawCommand2D::ImageBatch { .. } | DrawCommand2D::SpriteBatch { .. }
                    ) {
                        stats.batches = stats.batches.saturating_add(1);
                    }
                    stats.drawn = stats.drawn.saturating_add(1);
                    draws.push(draw);
                }
                NodePreparation::Group => {}
                NodePreparation::Culled => stats.culled = stats.culled.saturating_add(1),
                NodePreparation::Rejected => stats.rejected = stats.rejected.saturating_add(1),
            }
        }
        draws.sort_by(|left, right| {
            left.metadata()
                .layer
                .cmp(&right.metadata().layer)
                .then_with(|| {
                    left.metadata()
                        .source_index
                        .cmp(&right.metadata().source_index)
                })
        });
        stats.retained_resources = retained.len().try_into().unwrap_or(u32::MAX);

        PreparedScene2D {
            scene_to_view,
            view_to_scene,
            draws,
            diagnostics,
            stats,
        }
    }
}

enum NodePreparation {
    Draw(DrawCommand2D),
    Group,
    Culled,
    Rejected,
}

#[allow(clippy::too_many_arguments)]
fn prepare_node(
    scene: &Scene2DIR,
    node: &crate::Node2D,
    source_index: usize,
    world: &WorldNode,
    scene_to_view: Affine2,
    scene_view_bounds: Bounds2,
    viewport_bounds: Bounds2,
    diagnostics: &mut Vec<SceneDiagnostic>,
    retained: &mut BTreeSet<(u8, u64)>,
) -> NodePreparation {
    if matches!(node.content, NodeContent2D::Group) {
        return NodePreparation::Group;
    }
    if let NodeContent2D::ImageBatch(batch) = &node.content {
        return prepare_image_batch(
            scene,
            node,
            source_index,
            world,
            scene_to_view,
            viewport_bounds,
            batch,
            diagnostics,
            retained,
        );
    }
    if let NodeContent2D::SpriteBatch(batch) = &node.content {
        return prepare_sprite_batch(
            scene,
            node,
            source_index,
            world,
            scene_to_view,
            viewport_bounds,
            batch,
            diagnostics,
            retained,
        );
    }

    let Some(local_bounds) = content_bounds(scene, node, diagnostics, retained) else {
        return NodePreparation::Rejected;
    };
    let scene_bounds = world.scene_transform.transform_bounds(local_bounds);
    if !scene_bounds.intersects(scene_view_bounds)
        || world
            .clips
            .iter()
            .any(|(_, _, bounds)| !scene_bounds.intersects(*bounds))
    {
        return NodePreparation::Culled;
    }
    let view_transform = scene_to_view.then(world.scene_transform);
    let view_draw_bounds = view_transform.transform_bounds(local_bounds);
    if !view_draw_bounds.intersects(viewport_bounds) {
        return NodePreparation::Culled;
    }
    let metadata = metadata(
        node,
        source_index,
        world,
        scene_to_view,
        local_bounds,
        scene_bounds,
        view_draw_bounds,
    );
    let draw = match &node.content {
        NodeContent2D::Rectangle {
            rect,
            style,
            corner_radius,
        } => {
            if !valid_style(style) || !corner_radius.is_finite() || *corner_radius < 0.0 {
                diagnostics.push(invalid_node(
                    node.id,
                    "rectangle style or radius is invalid",
                ));
                return NodePreparation::Rejected;
            }
            DrawCommand2D::Rectangle {
                metadata,
                rect: *rect,
                style: style.clone(),
                corner_radius: *corner_radius,
            }
        }
        NodeContent2D::Path { path, style } => DrawCommand2D::Path {
            metadata,
            path: *path,
            style: style.clone(),
        },
        NodeContent2D::Text { text } => DrawCommand2D::Text {
            metadata,
            text: *text,
        },
        NodeContent2D::Image {
            image,
            destination,
            source,
            sampling,
            tint,
        } => DrawCommand2D::Image {
            metadata,
            image: *image,
            destination: *destination,
            source: *source,
            sampling: *sampling,
            tint: *tint,
        },
        NodeContent2D::Sprite(sprite) => {
            let sheet = &scene.resources.sprite_sheets[&sprite.sheet];
            let frame = sheet.frames[sprite.frame as usize];
            DrawCommand2D::Image {
                metadata,
                image: sheet.image,
                destination: sprite_destination(frame),
                source: Some(frame.source),
                sampling: sheet.sampling,
                tint: sprite.tint,
            }
        }
        NodeContent2D::Group | NodeContent2D::ImageBatch(_) | NodeContent2D::SpriteBatch(_) => {
            unreachable!()
        }
    };
    NodePreparation::Draw(draw)
}

#[allow(clippy::too_many_arguments)]
fn prepare_image_batch(
    scene: &Scene2DIR,
    node: &crate::Node2D,
    source_index: usize,
    world: &WorldNode,
    scene_to_view: Affine2,
    viewport_bounds: Bounds2,
    batch: &ImageBatch2D,
    diagnostics: &mut Vec<SceneDiagnostic>,
    retained: &mut BTreeSet<(u8, u64)>,
) -> NodePreparation {
    if let Some(diagnostic) = image_handle_error(scene, batch.image, Some(node.id)) {
        diagnostics.push(diagnostic);
        return NodePreparation::Rejected;
    }
    retained.insert((1, batch.image.id().get()));
    let mut instances = Vec::new();
    let mut union = None;
    for instance in &batch.instances {
        if !valid_image_instance(instance) {
            diagnostics.push(invalid_node(
                node.id,
                "image batch contains an invalid instance",
            ));
            continue;
        }
        let local = Affine2::from_transform(instance.transform);
        let view_transform = scene_to_view.then(world.scene_transform.then(local));
        let view_instance_bounds = view_transform.transform_bounds(instance.destination.bounds());
        if !view_instance_bounds.intersects(viewport_bounds)
            || world.clips.iter().any(|(_, _, bounds)| {
                !view_instance_bounds.intersects(scene_to_view.transform_bounds(*bounds))
            })
        {
            continue;
        }
        union = Some(union_bounds(union, view_instance_bounds));
        instances.push(PreparedImageInstance2D {
            view_transform,
            destination: instance.destination,
            source: instance.source,
            tint: instance.tint,
            opacity: world.opacity * instance.opacity,
            view_bounds: view_instance_bounds,
        });
    }
    let Some(view_draw_bounds) = union else {
        return NodePreparation::Culled;
    };
    let local_bounds = batch_bounds(&batch.instances);
    let scene_bounds = world.scene_transform.transform_bounds(local_bounds);
    NodePreparation::Draw(DrawCommand2D::ImageBatch {
        metadata: metadata(
            node,
            source_index,
            world,
            scene_to_view,
            local_bounds,
            scene_bounds,
            view_draw_bounds,
        ),
        image: batch.image,
        sampling: batch.sampling,
        instances,
    })
}

#[allow(clippy::too_many_arguments)]
fn prepare_sprite_batch(
    scene: &Scene2DIR,
    node: &crate::Node2D,
    source_index: usize,
    world: &WorldNode,
    scene_to_view: Affine2,
    viewport_bounds: Bounds2,
    batch: &crate::SpriteBatch2D,
    diagnostics: &mut Vec<SceneDiagnostic>,
    retained: &mut BTreeSet<(u8, u64)>,
) -> NodePreparation {
    let Some(sheet) = scene.resources.sprite_sheets.get(&batch.sheet) else {
        diagnostics.push(invalid_node(
            node.id,
            "sprite batch references a missing sheet",
        ));
        return NodePreparation::Rejected;
    };
    if let Some(diagnostic) = image_handle_error(scene, sheet.image, Some(node.id)) {
        diagnostics.push(diagnostic);
        return NodePreparation::Rejected;
    }
    retained.insert((3, batch.sheet.0));
    retained.insert((1, sheet.image.id().get()));
    let mut instances = Vec::new();
    let mut union = None;
    let mut local_union = None;
    for instance in &batch.instances {
        let Some(frame) = sheet.frames.get(instance.frame as usize).copied() else {
            diagnostics.push(invalid_node(
                node.id,
                format!("sprite frame {} does not exist", instance.frame),
            ));
            continue;
        };
        if !instance.transform.is_valid()
            || !instance.tint.is_valid()
            || !valid_opacity(instance.opacity)
        {
            diagnostics.push(invalid_node(
                node.id,
                "sprite batch contains an invalid instance",
            ));
            continue;
        }
        let destination = sprite_destination(frame);
        let local = Affine2::from_transform(instance.transform);
        let view_transform = scene_to_view.then(world.scene_transform.then(local));
        let view_instance_bounds = view_transform.transform_bounds(destination.bounds());
        if !view_instance_bounds.intersects(viewport_bounds) {
            continue;
        }
        union = Some(union_bounds(union, view_instance_bounds));
        local_union = Some(union_bounds(
            local_union,
            local.transform_bounds(destination.bounds()),
        ));
        instances.push(PreparedSpriteInstance2D {
            view_transform,
            destination,
            source: frame.source,
            tint: instance.tint,
            opacity: world.opacity * instance.opacity,
            view_bounds: view_instance_bounds,
        });
    }
    let (Some(view_draw_bounds), Some(local_bounds)) = (union, local_union) else {
        return NodePreparation::Culled;
    };
    let scene_bounds = world.scene_transform.transform_bounds(local_bounds);
    NodePreparation::Draw(DrawCommand2D::SpriteBatch {
        metadata: metadata(
            node,
            source_index,
            world,
            scene_to_view,
            local_bounds,
            scene_bounds,
            view_draw_bounds,
        ),
        image: sheet.image,
        sampling: sheet.sampling,
        instances,
    })
}

fn resolve_world(
    scene: &Scene2DIR,
    index: usize,
    by_id: &BTreeMap<NodeId, usize>,
    duplicate_ids: &BTreeSet<NodeId>,
    visit: &mut [u8],
    output: &mut [Option<WorldNode>],
    diagnostics: &mut Vec<SceneDiagnostic>,
) -> Option<WorldNode> {
    if visit[index] == 2 {
        return output[index].clone();
    }
    let node = &scene.nodes[index];
    if duplicate_ids.contains(&node.id) {
        visit[index] = 2;
        return None;
    }
    if visit[index] == 1 {
        diagnostics.push(
            SceneDiagnostic::error(
                "scene2d.hierarchy-cycle",
                format!("node {} participates in a parent cycle", node.id.get()),
            )
            .for_node(node.id),
        );
        return None;
    }
    visit[index] = 1;
    if !node.transform.is_valid() || !valid_opacity(node.opacity) {
        diagnostics.push(invalid_node(node.id, "transform or opacity is invalid"));
        visit[index] = 2;
        return None;
    }
    let parent = match node.parent {
        Some(parent_id) => {
            if duplicate_ids.contains(&parent_id) {
                diagnostics.push(
                    SceneDiagnostic::error(
                        "scene2d.ambiguous-parent",
                        format!("parent node {} is duplicated", parent_id.get()),
                    )
                    .for_node(node.id),
                );
                visit[index] = 2;
                return None;
            }
            let Some(parent_index) = by_id.get(&parent_id).copied() else {
                diagnostics.push(
                    SceneDiagnostic::error(
                        "scene2d.missing-parent",
                        format!("parent node {} does not exist", parent_id.get()),
                    )
                    .for_node(node.id),
                );
                visit[index] = 2;
                return None;
            };
            resolve_world(
                scene,
                parent_index,
                by_id,
                duplicate_ids,
                visit,
                output,
                diagnostics,
            )
        }
        None => Some(WorldNode {
            scene_transform: Affine2::IDENTITY,
            visible: true,
            opacity: 1.0,
            clips: Vec::new(),
        }),
    };
    let Some(parent) = parent else {
        visit[index] = 2;
        return None;
    };
    let scene_transform = parent
        .scene_transform
        .then(Affine2::from_transform(node.transform));
    let mut clips = parent.clips;
    if let Some(clip) = &node.clip {
        let Some(bounds) = clip_bounds(scene, clip) else {
            diagnostics.push(invalid_node(node.id, "clip references invalid geometry"));
            visit[index] = 2;
            return None;
        };
        clips.push((
            clip.clone(),
            scene_transform,
            scene_transform.transform_bounds(bounds),
        ));
    }
    let resolved = WorldNode {
        scene_transform,
        visible: parent.visible && node.visible,
        opacity: parent.opacity * node.opacity,
        clips,
    };
    visit[index] = 2;
    output[index] = Some(resolved.clone());
    Some(resolved)
}

fn content_bounds(
    scene: &Scene2DIR,
    node: &crate::Node2D,
    diagnostics: &mut Vec<SceneDiagnostic>,
    retained: &mut BTreeSet<(u8, u64)>,
) -> Option<Bounds2> {
    match &node.content {
        NodeContent2D::Group => None,
        NodeContent2D::Rectangle { rect, .. } => {
            rect.is_valid().then(|| rect.bounds()).or_else(|| {
                diagnostics.push(invalid_node(node.id, "rectangle geometry is invalid"));
                None
            })
        }
        NodeContent2D::Path { path, style } => {
            let resource = scene.resources.paths.get(path);
            if resource.is_none()
                || resource.is_some_and(|path| {
                    !path.bounds.is_valid() || path.commands.is_empty() || !path_is_finite(path)
                })
                || !valid_style(style)
            {
                diagnostics.push(invalid_node(node.id, "path resource or style is invalid"));
                return None;
            }
            retained.insert((2, path.0));
            Some(resource?.bounds)
        }
        NodeContent2D::Text { text } => {
            let resource = scene.resources.text.get(text);
            if resource.is_none()
                || resource.is_some_and(|text| {
                    text.font_key.trim().is_empty()
                        || !text.font_size.is_finite()
                        || text.font_size <= 0.0
                        || !text.bounds.is_valid()
                        || !text.color.is_valid()
                })
            {
                diagnostics.push(invalid_node(node.id, "text resource does not exist"));
                return None;
            }
            retained.insert((4, text.0));
            Some(resource?.bounds)
        }
        NodeContent2D::Image {
            image,
            destination,
            source,
            tint,
            ..
        } => {
            if !destination.is_valid()
                || source.is_some_and(|source| !source.rect.is_valid())
                || !tint.is_valid()
            {
                diagnostics.push(invalid_node(node.id, "image geometry or tint is invalid"));
                return None;
            }
            if let Some(diagnostic) = image_handle_error(scene, *image, Some(node.id)) {
                diagnostics.push(diagnostic);
                return None;
            }
            retained.insert((1, image.id().get()));
            Some(destination.bounds())
        }
        NodeContent2D::Sprite(sprite) => {
            let Some(sheet) = scene.resources.sprite_sheets.get(&sprite.sheet) else {
                diagnostics.push(invalid_node(node.id, "sprite sheet does not exist"));
                return None;
            };
            let Some(frame) = sheet.frames.get(sprite.frame as usize).copied() else {
                diagnostics.push(invalid_node(node.id, "sprite frame does not exist"));
                return None;
            };
            if !sprite.tint.is_valid() {
                diagnostics.push(invalid_node(node.id, "sprite tint is invalid"));
                return None;
            }
            if let Some(diagnostic) = image_handle_error(scene, sheet.image, Some(node.id)) {
                diagnostics.push(diagnostic);
                return None;
            }
            retained.insert((3, sprite.sheet.0));
            retained.insert((1, sheet.image.id().get()));
            Some(sprite_destination(frame).bounds())
        }
        NodeContent2D::ImageBatch(_) | NodeContent2D::SpriteBatch(_) => unreachable!(),
    }
}

fn metadata(
    node: &crate::Node2D,
    source_index: usize,
    world: &WorldNode,
    scene_to_view: Affine2,
    local_bounds: Bounds2,
    scene_bounds: Bounds2,
    view_bounds: Bounds2,
) -> DrawMetadata2D {
    DrawMetadata2D {
        node: node.id,
        source_index: source_index.try_into().unwrap_or(u32::MAX),
        layer: node.layer,
        blend_mode: node.blend_mode,
        opacity: world.opacity,
        scene_transform: world.scene_transform,
        view_transform: scene_to_view.then(world.scene_transform),
        local_bounds,
        scene_bounds,
        view_bounds,
        clips: world
            .clips
            .iter()
            .map(|(shape, transform, bounds)| PreparedClip2D {
                shape: shape.clone(),
                view_transform: scene_to_view.then(*transform),
                view_bounds: scene_to_view.transform_bounds(*bounds),
            })
            .collect(),
        interaction: node.interaction.clone(),
    }
}

fn camera_transform(scene: &Scene2DIR) -> Affine2 {
    let camera = scene.camera;
    let viewport = scene.viewport.rect;
    if !camera.is_valid() || !viewport.is_valid() {
        return Affine2::IDENTITY;
    }
    let (sin, cos) = (-camera.rotation_radians).sin_cos();
    let m11 = cos * camera.zoom;
    let m12 = sin * camera.zoom;
    let m21 = -sin * camera.zoom;
    let m22 = cos * camera.zoom;
    let center = Vec2::new(
        viewport.origin.x + viewport.size.x * 0.5,
        viewport.origin.y + viewport.size.y * 0.5,
    );
    Affine2 {
        m11,
        m12,
        m21,
        m22,
        tx: center.x - m11 * camera.center.x - m21 * camera.center.y,
        ty: center.y - m12 * camera.center.x - m22 * camera.center.y,
    }
}

fn validate_resources(scene: &Scene2DIR, diagnostics: &mut Vec<SceneDiagnostic>) {
    for (id, path) in &scene.resources.paths {
        if !path.bounds.is_valid() || path.commands.is_empty() || !path_is_finite(path) {
            diagnostics.push(SceneDiagnostic::error(
                "scene2d.invalid-path",
                format!("path resource {} has invalid commands or bounds", id.0),
            ));
        }
    }
    for (id, text) in &scene.resources.text {
        if text.font_key.trim().is_empty()
            || !text.font_size.is_finite()
            || text.font_size <= 0.0
            || !text.bounds.is_valid()
            || !text.color.is_valid()
            || text.glyphs.iter().any(|glyph| {
                !glyph.position.is_finite() || !glyph.advance.is_finite() || glyph.advance < 0.0
            })
        {
            diagnostics.push(SceneDiagnostic::error(
                "scene2d.invalid-text",
                format!("text resource {} is not valid resolved text", id.0),
            ));
        }
    }
    for (id, sheet) in &scene.resources.sprite_sheets {
        if sheet.frames.is_empty()
            || sheet.frames.iter().any(|frame| {
                !frame.source.rect.is_valid()
                    || !frame.size.is_finite()
                    || frame.size.x < 0.0
                    || frame.size.y < 0.0
                    || !frame.pivot.is_finite()
            })
        {
            diagnostics.push(SceneDiagnostic::error(
                "scene2d.invalid-sprite-sheet",
                format!("sprite sheet resource {} contains invalid frames", id.0),
            ));
        }
        if let Some(diagnostic) = image_handle_error(scene, sheet.image, None) {
            diagnostics.push(diagnostic);
        }
    }
}

fn image_handle_error(
    scene: &Scene2DIR,
    image: ImageHandle2D,
    node: Option<NodeId>,
) -> Option<SceneDiagnostic> {
    let asset = image.id();
    let (code, message) = if scene.assets.format_version
        != fission_scene::AssetBundle::FORMAT_VERSION
        || scene.assets.id.trim().is_empty()
    {
        (
            "scene2d.unusable-asset-bundle",
            format!(
                "image asset {} cannot be resolved because the scene asset bundle is invalid",
                asset.get()
            ),
        )
    } else if let Some(descriptor) = scene.assets.assets.iter().find(|entry| entry.id == asset) {
        if descriptor.kind != AssetKind::Image {
            (
                "scene2d.wrong-asset-kind",
                format!(
                    "asset {} is {:?}; a 2D image handle requires an image asset",
                    asset.get(),
                    descriptor.kind
                ),
            )
        } else if descriptor.source.trim().is_empty() || !valid_sha256(&descriptor.sha256) {
            (
                "scene2d.invalid-image-asset",
                format!(
                    "image asset {} must name a source and a lowercase SHA-256 digest",
                    asset.get()
                ),
            )
        } else {
            return None;
        }
    } else {
        (
            "scene2d.missing-image-asset",
            format!(
                "image asset {} is not present in asset bundle '{}'",
                asset.get(),
                scene.assets.id
            ),
        )
    };
    let diagnostic = SceneDiagnostic::error(code, message).for_asset(asset);
    Some(node.map_or(diagnostic.clone(), |node| diagnostic.for_node(node)))
}

fn valid_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn path_is_finite(path: &PathResource2D) -> bool {
    path.commands.iter().all(|command| match command {
        crate::PathCommand2D::MoveTo { point } | crate::PathCommand2D::LineTo { point } => {
            point.is_finite()
        }
        crate::PathCommand2D::QuadraticTo { control, point } => {
            control.is_finite() && point.is_finite()
        }
        crate::PathCommand2D::CubicTo {
            control1,
            control2,
            point,
        } => control1.is_finite() && control2.is_finite() && point.is_finite(),
        crate::PathCommand2D::Close => true,
    })
}

fn clip_bounds(scene: &Scene2DIR, clip: &Clip2D) -> Option<Bounds2> {
    match clip {
        Clip2D::Rectangle { rect } => rect.is_valid().then(|| rect.bounds()),
        Clip2D::Path { path } => scene.resources.paths.get(path).map(|path| path.bounds),
    }
}

fn valid_style(style: &PathStyle2D) -> bool {
    style.fill.is_none_or(|fill| fill.color.is_valid())
        && style.stroke.is_none_or(valid_stroke)
        && (style.fill.is_some() || style.stroke.is_some())
}

fn valid_stroke(stroke: Stroke2D) -> bool {
    stroke.color.is_valid() && stroke.width.is_finite() && stroke.width > 0.0
}

fn valid_image_instance(instance: &ImageInstance2D) -> bool {
    instance.transform.is_valid()
        && instance.destination.is_valid()
        && instance.source.is_none_or(|source| source.rect.is_valid())
        && instance.tint.is_valid()
        && valid_opacity(instance.opacity)
}

fn valid_opacity(opacity: f32) -> bool {
    opacity.is_finite() && (0.0..=1.0).contains(&opacity)
}

fn sprite_destination(frame: SpriteFrame2D) -> Rect2D {
    Rect2D::new(Vec2::new(-frame.pivot.x, -frame.pivot.y), frame.size)
}

fn batch_bounds(instances: &[ImageInstance2D]) -> Bounds2 {
    instances
        .iter()
        .filter(|instance| instance.transform.is_valid() && instance.destination.is_valid())
        .fold(None, |bounds, instance| {
            let transformed = Affine2::from_transform(instance.transform)
                .transform_bounds(instance.destination.bounds());
            Some(union_bounds(bounds, transformed))
        })
        .unwrap_or_default()
}

fn union_bounds(existing: Option<Bounds2>, next: Bounds2) -> Bounds2 {
    existing.map_or(next, |existing| {
        Bounds2::new(
            Vec2::new(
                existing.min.x.min(next.min.x),
                existing.min.y.min(next.min.y),
            ),
            Vec2::new(
                existing.max.x.max(next.max.x),
                existing.max.y.max(next.max.y),
            ),
        )
    })
}

fn invalid_node(node: NodeId, message: impl Into<String>) -> SceneDiagnostic {
    SceneDiagnostic::error("scene2d.invalid-node", message).for_node(node)
}
