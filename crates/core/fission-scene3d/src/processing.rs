use std::collections::{BTreeMap, BTreeSet};

use fission_scene::{AssetKind, Bounds3, NodeId, SceneDiagnostic, ScenePassStats, Vec3};
use serde::{Deserialize, Serialize};

use crate::geometry::{transform_bounds, transform_matrix, vec3};
use crate::{
    AlphaMode3D, CameraProjection3D, Light3D, MaterialModel3D, NodeContent3D, Primitive3D,
    ResourceId, Scene3DIR, SCENE3D_FORMAT_VERSION,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RenderCapabilities3D {
    pub textures: bool,
    pub metallic_roughness: bool,
    pub alpha_blending: bool,
    pub point_lights: bool,
}

impl Default for RenderCapabilities3D {
    fn default() -> Self {
        Self {
            textures: true,
            metallic_roughness: true,
            alpha_blending: true,
            point_lights: true,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PreparedNode3D {
    pub id: NodeId,
    pub source_order: u32,
    pub world_transform: [f32; 16],
    pub world_bounds: Bounds3,
    pub content: NodeContent3D,
    pub material: Option<ResourceId>,
    pub blend_order: i32,
    pub pickable: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DrawCommand3D {
    pub node: NodeId,
    pub source_order: u32,
    pub content: NodeContent3D,
    pub material: Option<ResourceId>,
    pub world_transform: [f32; 16],
    pub transparent: bool,
    pub blend_order: i32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PreparedScene3D {
    pub source: Scene3DIR,
    pub nodes: Vec<PreparedNode3D>,
    pub draws: Vec<DrawCommand3D>,
    pub diagnostics: Vec<SceneDiagnostic>,
    pub stats: ScenePassStats,
}

impl PreparedScene3D {
    pub fn is_renderable(&self) -> bool {
        !self.draws.is_empty()
            && !self
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.severity == fission_scene::SceneSeverity::Error)
    }
}

#[derive(Default)]
pub struct Scene3DProcessor {
    retained: BTreeMap<(u8, u64), u64>,
}

impl Scene3DProcessor {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn prepare(
        &mut self,
        scene: &Scene3DIR,
        capabilities: RenderCapabilities3D,
    ) -> PreparedScene3D {
        let mut diagnostics = Vec::new();
        let mut stats = ScenePassStats::default();
        validate_scene(scene, capabilities, &mut diagnostics);
        account_resources(scene, &mut self.retained, &mut stats);

        let duplicates = duplicate_nodes(scene);
        for id in &duplicates {
            diagnostics.push(
                SceneDiagnostic::error("scene3d.duplicate-node", "node identity is duplicated")
                    .for_node(*id),
            );
        }
        let declarations = scene
            .nodes
            .iter()
            .filter(|node| !duplicates.contains(&node.id))
            .map(|node| (node.id, node))
            .collect::<BTreeMap<_, _>>();
        let mut resolved = BTreeMap::<NodeId, (glam::Mat4, bool)>::new();
        let mut visiting = BTreeSet::new();
        let view_projection = camera_view_projection(scene);
        let mut nodes = Vec::new();
        let mut draws = Vec::new();

        for (source_order, node) in scene.nodes.iter().enumerate() {
            stats.considered += 1;
            if duplicates.contains(&node.id) {
                stats.rejected += 1;
                continue;
            }
            let Some((world, visible)) = resolve_node(
                node.id,
                &declarations,
                &mut resolved,
                &mut visiting,
                &mut diagnostics,
            ) else {
                stats.rejected += 1;
                continue;
            };
            let Some(local_bounds) = content_bounds(scene, node.content) else {
                if matches!(node.content, NodeContent3D::Group) {
                    continue;
                }
                diagnostics.push(
                    SceneDiagnostic::error(
                        "scene3d.missing-resource",
                        "node references a missing or invalid mesh/model resource",
                    )
                    .for_node(node.id),
                );
                stats.rejected += 1;
                continue;
            };
            let world_bounds = transform_bounds(local_bounds, world);
            if !visible || !bounds_in_frustum(world_bounds, view_projection) {
                stats.culled += 1;
                continue;
            }
            let material = node
                .material
                .and_then(|id| scene.resources.materials.get(&id));
            let transparent =
                material.is_some_and(|material| material.alpha_mode == AlphaMode3D::Blend);
            let prepared = PreparedNode3D {
                id: node.id,
                source_order: source_order as u32,
                world_transform: world.to_cols_array(),
                world_bounds,
                content: node.content,
                material: node.material,
                blend_order: node.blend_order,
                pickable: node.pickable,
            };
            draws.push(DrawCommand3D {
                node: node.id,
                source_order: source_order as u32,
                content: node.content,
                material: node.material,
                world_transform: world.to_cols_array(),
                transparent,
                blend_order: node.blend_order,
            });
            nodes.push(prepared);
        }

        draws.sort_by_key(|draw| (draw.transparent, draw.blend_order, draw.source_order));
        stats.drawn = draws.len() as u32;
        stats.batches = count_batches(&draws);

        PreparedScene3D {
            source: scene.clone(),
            nodes,
            draws,
            diagnostics,
            stats,
        }
    }
}

fn validate_scene(
    scene: &Scene3DIR,
    capabilities: RenderCapabilities3D,
    diagnostics: &mut Vec<SceneDiagnostic>,
) {
    if let Err(errors) = scene.assets.validate() {
        diagnostics.extend(errors.into_iter().map(|error| {
            let diagnostic = SceneDiagnostic::error(
                "scene3d.invalid-asset-bundle",
                format!("asset bundle '{}': {}", scene.assets.id, error.message),
            );
            error
                .asset
                .map_or(diagnostic.clone(), |asset| diagnostic.for_asset(asset))
        }));
    }
    if scene.format_version != SCENE3D_FORMAT_VERSION {
        diagnostics.push(SceneDiagnostic::error(
            "scene3d.unsupported-version",
            format!(
                "unsupported scene version {}; expected {}",
                scene.format_version, SCENE3D_FORMAT_VERSION
            ),
        ));
    }
    if !scene.viewport.is_valid() {
        diagnostics.push(SceneDiagnostic::error(
            "scene3d.invalid-viewport",
            "viewport size must be finite and positive",
        ));
    }
    if !scene.camera.is_valid() {
        diagnostics.push(SceneDiagnostic::error(
            "scene3d.invalid-camera",
            "camera vectors or projection are invalid",
        ));
    }
    if !scene.clear.color.is_valid()
        || scene
            .clear
            .depth
            .is_some_and(|depth| !depth.is_finite() || !(0.0..=1.0).contains(&depth))
    {
        diagnostics.push(SceneDiagnostic::error(
            "scene3d.invalid-clear",
            "clear color or depth is invalid",
        ));
    }
    for (id, mesh) in &scene.resources.meshes {
        if !mesh.bounds.is_valid()
            || mesh.vertices.is_empty()
            || mesh.indices.is_empty()
            || mesh.indices.len() % 3 != 0
            || mesh
                .indices
                .iter()
                .any(|index| *index as usize >= mesh.vertices.len())
            || mesh.vertices.iter().any(|vertex| {
                !vertex.position.is_finite() || !vertex.normal.is_finite() || !vertex.uv.is_finite()
            })
        {
            diagnostics.push(SceneDiagnostic::error(
                "scene3d.invalid-mesh",
                format!("mesh {} has invalid vertices, indices, or bounds", id.0),
            ));
        }
    }
    for (id, model) in &scene.resources.models {
        if !model.bounds.is_valid() {
            diagnostics.push(SceneDiagnostic::error(
                "scene3d.invalid-model",
                format!("model {} has invalid bounds", id.0),
            ));
        }
        for (index, node) in model.nodes.iter().enumerate() {
            if !node.transform.is_valid()
                || node.parent.is_some_and(|parent| {
                    parent as usize >= model.nodes.len() || parent as usize >= index
                })
                || node
                    .mesh
                    .is_some_and(|mesh| !scene.resources.meshes.contains_key(&mesh))
                || node
                    .material
                    .is_some_and(|material| !scene.resources.materials.contains_key(&material))
            {
                diagnostics.push(SceneDiagnostic::error(
                    "scene3d.invalid-model-node",
                    format!(
                        "model {} node {} has invalid graph or resource data",
                        id.0, index
                    ),
                ));
            }
        }
    }
    for (id, material) in &scene.resources.materials {
        let valid_model = match material.model {
            MaterialModel3D::Unlit => true,
            MaterialModel3D::MetallicRoughness {
                metallic,
                roughness,
            } => {
                metallic.is_finite()
                    && roughness.is_finite()
                    && (0.0..=1.0).contains(&metallic)
                    && (0.0..=1.0).contains(&roughness)
            }
        };
        if !valid_model
            || !material.base_color.is_valid()
            || !material.emissive_color.is_valid()
            || !material.alpha_cutoff.is_finite()
            || !(0.0..=1.0).contains(&material.alpha_cutoff)
        {
            diagnostics.push(SceneDiagnostic::error(
                "scene3d.invalid-material",
                format!("material {} contains an invalid value", id.0),
            ));
        }
        for texture in [material.base_color_texture, material.emissive_texture]
            .into_iter()
            .flatten()
        {
            if !scene.resources.textures.contains_key(&texture) {
                diagnostics.push(SceneDiagnostic::error(
                    "scene3d.missing-texture",
                    format!("material {} references missing texture {}", id.0, texture.0),
                ));
            }
        }
        if !capabilities.textures
            && (material.base_color_texture.is_some() || material.emissive_texture.is_some())
        {
            diagnostics.push(SceneDiagnostic::error(
                "scene3d.unsupported-textures",
                "renderer does not support textures required by this scene",
            ));
        }
        if !capabilities.metallic_roughness
            && matches!(material.model, MaterialModel3D::MetallicRoughness { .. })
        {
            diagnostics.push(SceneDiagnostic::error(
                "scene3d.unsupported-metallic-roughness",
                "renderer does not support metallic-roughness materials",
            ));
        }
        if !capabilities.alpha_blending && material.alpha_mode == AlphaMode3D::Blend {
            diagnostics.push(SceneDiagnostic::error(
                "scene3d.unsupported-alpha-blending",
                "renderer does not support alpha blending required by this scene",
            ));
        }
    }
    for (id, texture) in &scene.resources.textures {
        let asset = texture.asset.id();
        let Some(descriptor) = scene
            .assets
            .assets
            .iter()
            .find(|descriptor| descriptor.id == asset)
        else {
            diagnostics.push(
                SceneDiagnostic::error(
                    "scene3d.missing-texture-asset",
                    format!(
                        "texture {} references asset {} which is absent from bundle '{}'",
                        id.0,
                        asset.get(),
                        scene.assets.id
                    ),
                )
                .for_asset(asset),
            );
            continue;
        };
        if descriptor.kind != AssetKind::Texture && descriptor.kind != AssetKind::Image {
            diagnostics.push(
                SceneDiagnostic::error(
                    "scene3d.wrong-texture-asset-kind",
                    format!(
                        "texture {} references {:?} asset {}; expected image or texture",
                        id.0,
                        descriptor.kind,
                        asset.get()
                    ),
                )
                .for_asset(asset),
            );
        }
    }
    for light in &scene.lights {
        let valid = match light {
            Light3D::Ambient(light) => light.color.is_valid() && valid_intensity(light.intensity),
            Light3D::Directional(light) => {
                light.direction.is_finite()
                    && vec3(light.direction).length_squared() > f32::EPSILON
                    && light.color.is_valid()
                    && valid_intensity(light.intensity)
            }
            Light3D::Point(light) => {
                light.position.is_finite()
                    && light.color.is_valid()
                    && valid_intensity(light.intensity)
                    && light.range.is_finite()
                    && light.range > 0.0
            }
        };
        if !valid {
            diagnostics.push(SceneDiagnostic::error(
                "scene3d.invalid-light",
                "light contains a non-finite or out-of-range value",
            ));
        }
        if matches!(light, Light3D::Point(_)) && !capabilities.point_lights {
            diagnostics.push(SceneDiagnostic::error(
                "scene3d.unsupported-point-light",
                "renderer does not support point lights required by this scene",
            ));
        }
    }
}

fn valid_intensity(value: f32) -> bool {
    value.is_finite() && value >= 0.0
}

fn duplicate_nodes(scene: &Scene3DIR) -> BTreeSet<NodeId> {
    let mut seen = BTreeSet::new();
    let mut duplicate = BTreeSet::new();
    for node in &scene.nodes {
        if !seen.insert(node.id) {
            duplicate.insert(node.id);
        }
    }
    duplicate
}

fn resolve_node(
    id: NodeId,
    declarations: &BTreeMap<NodeId, &crate::Node3D>,
    resolved: &mut BTreeMap<NodeId, (glam::Mat4, bool)>,
    visiting: &mut BTreeSet<NodeId>,
    diagnostics: &mut Vec<SceneDiagnostic>,
) -> Option<(glam::Mat4, bool)> {
    if let Some(value) = resolved.get(&id) {
        return Some(*value);
    }
    if !visiting.insert(id) {
        diagnostics.push(
            SceneDiagnostic::error("scene3d.parent-cycle", "node parent graph contains a cycle")
                .for_node(id),
        );
        return None;
    }
    let node = declarations[&id];
    let Some(local) = transform_matrix(node.transform) else {
        diagnostics.push(
            SceneDiagnostic::error("scene3d.invalid-transform", "node transform is invalid")
                .for_node(id),
        );
        visiting.remove(&id);
        return None;
    };
    let value = if let Some(parent) = node.parent {
        let Some(_) = declarations.get(&parent) else {
            diagnostics.push(
                SceneDiagnostic::error("scene3d.missing-parent", "node parent does not exist")
                    .for_node(id),
            );
            visiting.remove(&id);
            return None;
        };
        let Some((parent_world, parent_visible)) =
            resolve_node(parent, declarations, resolved, visiting, diagnostics)
        else {
            visiting.remove(&id);
            return None;
        };
        (parent_world * local, parent_visible && node.visible)
    } else {
        (local, node.visible)
    };
    visiting.remove(&id);
    resolved.insert(id, value);
    Some(value)
}

fn content_bounds(scene: &Scene3DIR, content: NodeContent3D) -> Option<Bounds3> {
    match content {
        NodeContent3D::Group => None,
        NodeContent3D::Primitive(Primitive3D::Cube { size }) => {
            if !size.is_finite() || size.x <= 0.0 || size.y <= 0.0 || size.z <= 0.0 {
                return None;
            }
            let half = Vec3::new(size.x * 0.5, size.y * 0.5, size.z * 0.5);
            Some(Bounds3::new(Vec3::new(-half.x, -half.y, -half.z), half))
        }
        NodeContent3D::Primitive(Primitive3D::Sphere { radius }) => {
            if !radius.is_finite() || radius <= 0.0 {
                return None;
            }
            Some(Bounds3::new(
                Vec3::new(-radius, -radius, -radius),
                Vec3::new(radius, radius, radius),
            ))
        }
        NodeContent3D::Primitive(Primitive3D::Mesh { mesh }) => scene
            .resources
            .meshes
            .get(&mesh)
            .filter(|mesh| mesh.bounds.is_valid())
            .map(|mesh| mesh.bounds),
        NodeContent3D::Model { model } => scene
            .resources
            .models
            .get(&model)
            .filter(|model| model.bounds.is_valid())
            .map(|model| model.bounds),
    }
}

fn camera_view_projection(scene: &Scene3DIR) -> Option<glam::Mat4> {
    if !scene.camera.is_valid() || !scene.viewport.is_valid() {
        return None;
    }
    let view = glam::Mat4::look_at_rh(
        vec3(scene.camera.eye),
        vec3(scene.camera.target),
        vec3(scene.camera.up).normalize(),
    );
    let aspect = scene.viewport.size.x / scene.viewport.size.y;
    let projection = match scene.camera.projection {
        CameraProjection3D::Perspective {
            vertical_fov_radians,
            near,
            far,
        } => glam::Mat4::perspective_rh(vertical_fov_radians, aspect, near, far),
        CameraProjection3D::Orthographic {
            vertical_size,
            near,
            far,
        } => {
            let half_height = vertical_size * 0.5;
            let half_width = half_height * aspect;
            glam::Mat4::orthographic_rh(
                -half_width,
                half_width,
                -half_height,
                half_height,
                near,
                far,
            )
        }
    };
    Some(projection * view)
}

fn bounds_in_frustum(bounds: Bounds3, view_projection: Option<glam::Mat4>) -> bool {
    let Some(matrix) = view_projection else {
        return true;
    };
    let min = vec3(bounds.min);
    let max = vec3(bounds.max);
    let corners = [
        glam::Vec3::new(min.x, min.y, min.z),
        glam::Vec3::new(min.x, min.y, max.z),
        glam::Vec3::new(min.x, max.y, min.z),
        glam::Vec3::new(min.x, max.y, max.z),
        glam::Vec3::new(max.x, min.y, min.z),
        glam::Vec3::new(max.x, min.y, max.z),
        glam::Vec3::new(max.x, max.y, min.z),
        glam::Vec3::new(max.x, max.y, max.z),
    ];
    let clips = corners.map(|corner| matrix * corner.extend(1.0));
    ![
        (0, -1.0),
        (0, 1.0),
        (1, -1.0),
        (1, 1.0),
        (2, -1.0),
        (2, 1.0),
    ]
    .into_iter()
    .any(|(axis, sign)| {
        clips.iter().all(|clip| {
            if sign < 0.0 {
                clip[axis] < -clip.w
            } else {
                clip[axis] > clip.w
            }
        })
    })
}

fn account_resources(
    scene: &Scene3DIR,
    retained: &mut BTreeMap<(u8, u64), u64>,
    stats: &mut ScenePassStats,
) {
    let resources = scene
        .resources
        .meshes
        .iter()
        .map(|(id, resource)| ((0, id.0), resource.revision))
        .chain(
            scene
                .resources
                .textures
                .iter()
                .map(|(id, resource)| ((1, id.0), resource.revision)),
        )
        .chain(
            scene
                .resources
                .materials
                .iter()
                .map(|(id, resource)| ((2, id.0), resource.revision)),
        )
        .chain(
            scene
                .resources
                .models
                .iter()
                .map(|(id, resource)| ((3, id.0), resource.revision)),
        )
        .collect::<Vec<_>>();
    let active = resources
        .iter()
        .map(|(key, _)| *key)
        .collect::<BTreeSet<_>>();
    retained.retain(|key, _| active.contains(key));
    for (key, revision) in resources {
        if retained.get(&key) == Some(&revision) {
            stats.retained_resources += 1;
        } else {
            retained.insert(key, revision);
            stats.uploaded_resources += 1;
        }
    }
}

fn count_batches(draws: &[DrawCommand3D]) -> u32 {
    let mut previous = None;
    let mut batches = 0;
    for draw in draws {
        let mesh = match draw.content {
            NodeContent3D::Primitive(Primitive3D::Mesh { mesh }) => Some(mesh),
            _ => None,
        };
        let key = (mesh, draw.material, draw.transparent, draw.blend_order);
        if previous != Some(key) {
            batches += 1;
            previous = Some(key);
        }
    }
    batches
}

#[cfg(test)]
mod tests {
    use fission_scene::{
        AssetDescriptor, AssetHandle, AssetId, AssetKind, NodeId, SceneId, TextureAsset,
        Transform3, Vec3,
    };

    use super::*;
    use crate::{Node3D, Primitive3D, Viewport3D};

    #[test]
    fn resolves_hierarchy_culls_and_reuses_resources() {
        let mut scene = Scene3DIR::new(SceneId(1), Viewport3D::new(800.0, 600.0));
        scene.camera = crate::Camera3D::perspective(
            Vec3::new(0.0, 0.0, 8.0),
            Vec3::ZERO,
            60.0_f32.to_radians(),
        );
        let parent = NodeId(1);
        let mut root = Node3D::group(parent);
        root.transform.translation = Vec3::new(1.0, 0.0, 0.0);
        scene.nodes.push(root);
        scene.nodes.push(Node3D {
            id: NodeId(2),
            parent: Some(parent),
            transform: Transform3::IDENTITY,
            visible: true,
            content: NodeContent3D::Primitive(Primitive3D::Cube { size: Vec3::ONE }),
            material: None,
            blend_order: 0,
            pickable: true,
        });
        scene.nodes.push(Node3D {
            id: NodeId(3),
            parent: None,
            transform: Transform3 {
                translation: Vec3::new(10_000.0, 0.0, 0.0),
                ..Transform3::IDENTITY
            },
            visible: true,
            content: NodeContent3D::Primitive(Primitive3D::Sphere { radius: 1.0 }),
            material: None,
            blend_order: 0,
            pickable: true,
        });

        let mut processor = Scene3DProcessor::new();
        let prepared = processor.prepare(&scene, RenderCapabilities3D::default());
        assert_eq!(prepared.draws.len(), 1);
        assert_eq!(prepared.stats.culled, 1);
        assert_eq!(prepared.nodes[0].world_bounds.min.x, 0.5);
    }

    #[test]
    fn invalid_graph_and_renderer_capabilities_are_actionable() {
        let mut scene = Scene3DIR::new(SceneId(1), Viewport3D::new(10.0, 10.0));
        scene.nodes.push(Node3D {
            parent: Some(NodeId(99)),
            ..Node3D::group(NodeId(1))
        });
        let mut processor = Scene3DProcessor::new();
        let prepared = processor.prepare(&scene, RenderCapabilities3D::default());
        assert!(prepared
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "scene3d.missing-parent"));
    }

    #[test]
    fn texture_handles_require_packaged_sources_of_the_right_kind() {
        let mut scene = Scene3DIR::new(SceneId(8), Viewport3D::new(10.0, 10.0));
        scene.resources.textures.insert(
            crate::TextureId(3),
            crate::Texture3D {
                revision: 1,
                asset: AssetHandle::<TextureAsset>::new(AssetId(44)),
                sampling: crate::TextureSampling3D::Linear,
                srgb: true,
            },
        );
        let missing = Scene3DProcessor::new().prepare(&scene, RenderCapabilities3D::default());
        assert!(missing
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "scene3d.missing-texture-asset"));

        scene.assets.assets.push(AssetDescriptor {
            id: AssetId(44),
            kind: AssetKind::Model,
            source: "assets/cooked/albedo.png".into(),
            sha256: "a".repeat(64),
        });
        let wrong_kind = Scene3DProcessor::new().prepare(&scene, RenderCapabilities3D::default());
        assert!(wrong_kind
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "scene3d.wrong-texture-asset-kind"));
    }

    #[test]
    fn unchanged_resource_revisions_are_reused_across_presentations() {
        let mut scene = Scene3DIR::new(SceneId(1), Viewport3D::new(10.0, 10.0));
        scene
            .resources
            .materials
            .insert(ResourceId(7), crate::Material3D::default());
        let mut processor = Scene3DProcessor::new();

        let first = processor.prepare(&scene, RenderCapabilities3D::default());
        let second = processor.prepare(&scene, RenderCapabilities3D::default());
        scene
            .resources
            .materials
            .get_mut(&ResourceId(7))
            .unwrap()
            .revision = 1;
        let changed = processor.prepare(&scene, RenderCapabilities3D::default());

        assert_eq!(first.stats.uploaded_resources, 1);
        assert_eq!(second.stats.retained_resources, 1);
        assert_eq!(changed.stats.uploaded_resources, 1);
    }
}
