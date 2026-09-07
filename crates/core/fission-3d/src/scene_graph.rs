//! Backend-neutral retained node graph and transform-resolution pass.

use std::collections::{BTreeMap, BTreeSet};

use fission_ir::WidgetId;
use serde::{Deserialize, Serialize};

use crate::{Point3D, Primitive3D};

/// Stable identity for one retained node in a 3D scene.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Node3DId(WidgetId);

impl Node3DId {
    pub fn explicit(key: &str) -> Self {
        Self(WidgetId::explicit(key))
    }

    pub const fn from_u128(value: u128) -> Self {
        Self(WidgetId::from_u128(value))
    }

    pub fn as_u128(self) -> u128 {
        self.0.as_u128()
    }
}

/// Quaternion rotation stored in `(x, y, z, w)` order.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Rotation3D {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub w: f32,
}

impl Rotation3D {
    pub const IDENTITY: Self = Self::new(0.0, 0.0, 0.0, 1.0);

    pub const fn new(x: f32, y: f32, z: f32, w: f32) -> Self {
        Self { x, y, z, w }
    }

    pub fn is_valid(self) -> bool {
        self.as_quat().is_some()
    }

    fn as_quat(self) -> Option<glam::Quat> {
        let value = glam::Quat::from_xyzw(self.x, self.y, self.z, self.w);
        value
            .is_finite()
            .then_some(value)
            .and_then(|value| (value.length_squared() > f32::EPSILON).then(|| value.normalize()))
    }
}

impl Default for Rotation3D {
    fn default() -> Self {
        Self::IDENTITY
    }
}

/// Local translation, rotation, and scale for a retained scene node.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Transform3D {
    pub translation: Point3D,
    pub rotation: Rotation3D,
    pub scale: Point3D,
}

impl Transform3D {
    pub const IDENTITY: Self = Self {
        translation: Point3D::new(0.0, 0.0, 0.0),
        rotation: Rotation3D::IDENTITY,
        scale: Point3D::new(1.0, 1.0, 1.0),
    };

    pub const fn from_translation(translation: Point3D) -> Self {
        Self {
            translation,
            ..Self::IDENTITY
        }
    }

    /// Returns the column-major local matrix when every component is valid.
    pub fn matrix(self) -> Option<[f32; 16]> {
        self.as_mat4().map(|matrix| matrix.to_cols_array())
    }

    fn as_mat4(self) -> Option<glam::Mat4> {
        let translation =
            glam::Vec3::new(self.translation.x, self.translation.y, self.translation.z);
        let scale = glam::Vec3::new(self.scale.x, self.scale.y, self.scale.z);
        let rotation = self.rotation.as_quat()?;
        (translation.is_finite() && scale.is_finite())
            .then(|| glam::Mat4::from_scale_rotation_translation(scale, rotation, translation))
    }
}

impl Default for Transform3D {
    fn default() -> Self {
        Self::IDENTITY
    }
}

/// One retained scene node with an optional renderable primitive.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Node3D {
    pub id: Node3DId,
    pub parent: Option<Node3DId>,
    pub transform: Transform3D,
    pub primitive: Option<Primitive3D>,
    pub visible: bool,
}

impl Node3D {
    pub fn new(id: Node3DId) -> Self {
        Self {
            id,
            parent: None,
            transform: Transform3D::default(),
            primitive: None,
            visible: true,
        }
    }

    pub fn parent(mut self, parent: Node3DId) -> Self {
        self.parent = Some(parent);
        self
    }

    pub fn transform(mut self, transform: Transform3D) -> Self {
        self.transform = transform;
        self
    }

    pub fn primitive(mut self, primitive: Primitive3D) -> Self {
        self.primitive = Some(primitive);
        self
    }

    pub fn visible(mut self, visible: bool) -> Self {
        self.visible = visible;
        self
    }
}

/// Structural problem found while producing the closed scene IR.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Scene3DDiagnostic {
    DuplicateNodeId { id: Node3DId },
    InvalidTransform { id: Node3DId },
    InvalidPrimitive { id: Node3DId },
    MissingParent { id: Node3DId, parent: Node3DId },
    ParentCycle { id: Node3DId },
    UnresolvedParent { id: Node3DId, parent: Node3DId },
}

/// Retained node after local transforms have been resolved into world space.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ResolvedNode3D {
    pub id: Node3DId,
    pub world_transform: [f32; 16],
    pub primitive: Option<Primitive3D>,
    pub visible: bool,
}

/// Closed, backend-neutral output of the 3D structural and transform passes.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Scene3DIR {
    pub nodes: Vec<ResolvedNode3D>,
    pub diagnostics: Vec<Scene3DDiagnostic>,
}

pub(crate) fn resolve_nodes(nodes: &[Node3D]) -> Scene3DIR {
    let mut counts = BTreeMap::<Node3DId, usize>::new();
    for node in nodes {
        *counts.entry(node.id).or_default() += 1;
    }

    let duplicates = counts
        .iter()
        .filter_map(|(id, count)| (*count > 1).then_some(*id))
        .collect::<BTreeSet<_>>();
    let mut diagnostics = duplicates
        .iter()
        .map(|id| Scene3DDiagnostic::DuplicateNodeId { id: *id })
        .collect::<Vec<_>>();

    let declarations = nodes
        .iter()
        .filter(|node| !duplicates.contains(&node.id))
        .map(|node| (node.id, node))
        .collect::<BTreeMap<_, _>>();
    let mut resolved_transforms = BTreeMap::<Node3DId, (glam::Mat4, bool)>::new();
    let mut visiting = BTreeSet::<Node3DId>::new();
    let mut reported_cycles = BTreeSet::<Node3DId>::new();

    for node in nodes {
        if !duplicates.contains(&node.id) {
            resolve_node(
                node.id,
                &declarations,
                &mut resolved_transforms,
                &mut visiting,
                &mut reported_cycles,
                &mut diagnostics,
            );
        }
    }

    let mut resolved = Vec::new();
    for node in nodes {
        let Some((matrix, visible)) = resolved_transforms.get(&node.id) else {
            continue;
        };
        if node
            .primitive
            .as_ref()
            .is_some_and(|primitive| !primitive_is_valid(primitive))
        {
            diagnostics.push(Scene3DDiagnostic::InvalidPrimitive { id: node.id });
            continue;
        }
        resolved.push(ResolvedNode3D {
            id: node.id,
            world_transform: matrix.to_cols_array(),
            primitive: node.primitive.clone(),
            visible: *visible,
        });
    }

    Scene3DIR {
        nodes: resolved,
        diagnostics,
    }
}

fn primitive_is_valid(primitive: &Primitive3D) -> bool {
    match primitive {
        Primitive3D::Cube { center, size, .. } => {
            center.is_finite() && size.is_finite() && *size > 0.0
        }
        Primitive3D::Sphere { center, radius, .. } => {
            center.is_finite() && radius.is_finite() && *radius > 0.0
        }
        Primitive3D::Mesh {
            vertices, indices, ..
        } => {
            !vertices.is_empty()
                && vertices.iter().all(Point3D::is_finite)
                && !indices.is_empty()
                && indices.len() % 3 == 0
                && indices
                    .iter()
                    .all(|index| (*index as usize) < vertices.len())
        }
    }
}

fn resolve_node(
    id: Node3DId,
    declarations: &BTreeMap<Node3DId, &Node3D>,
    resolved_transforms: &mut BTreeMap<Node3DId, (glam::Mat4, bool)>,
    visiting: &mut BTreeSet<Node3DId>,
    reported_cycles: &mut BTreeSet<Node3DId>,
    diagnostics: &mut Vec<Scene3DDiagnostic>,
) -> Option<(glam::Mat4, bool)> {
    if let Some(resolved) = resolved_transforms.get(&id) {
        return Some(*resolved);
    }
    if !visiting.insert(id) {
        if reported_cycles.insert(id) {
            diagnostics.push(Scene3DDiagnostic::ParentCycle { id });
        }
        return None;
    }

    let node = declarations[&id];
    let Some(local) = node.transform.as_mat4() else {
        diagnostics.push(Scene3DDiagnostic::InvalidTransform { id });
        visiting.remove(&id);
        return None;
    };
    let (world, visible) = if let Some(parent) = node.parent {
        let Some(_) = declarations.get(&parent) else {
            diagnostics.push(Scene3DDiagnostic::MissingParent { id, parent });
            visiting.remove(&id);
            return None;
        };
        let Some((parent_matrix, parent_visible)) = resolve_node(
            parent,
            declarations,
            resolved_transforms,
            visiting,
            reported_cycles,
            diagnostics,
        ) else {
            diagnostics.push(Scene3DDiagnostic::UnresolvedParent { id, parent });
            visiting.remove(&id);
            return None;
        };
        (parent_matrix * local, parent_visible && node.visible)
    } else {
        (local, node.visible)
    };

    visiting.remove(&id);
    resolved_transforms.insert(id, (world, visible));
    Some((world, visible))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(name: &str) -> Node3DId {
        Node3DId::explicit(name)
    }

    #[test]
    fn hierarchy_resolves_parent_before_child_without_reordering_output() {
        let child = Node3D::new(id("child"))
            .parent(id("parent"))
            .transform(Transform3D::from_translation(Point3D::new(0.0, 3.0, 0.0)));
        let parent = Node3D::new(id("parent"))
            .transform(Transform3D::from_translation(Point3D::new(2.0, 0.0, 0.0)))
            .visible(false);

        let ir = resolve_nodes(&[child, parent]);

        assert!(ir.diagnostics.is_empty(), "{:?}", ir.diagnostics);
        assert_eq!(ir.nodes[0].id, id("child"));
        assert!(!ir.nodes[0].visible, "parent visibility must be inherited");
        let transform = glam::Mat4::from_cols_array(&ir.nodes[0].world_transform);
        assert_eq!(
            transform.transform_point3(glam::Vec3::ZERO),
            glam::vec3(2.0, 3.0, 0.0)
        );
    }

    #[test]
    fn duplicate_missing_and_cyclic_nodes_are_diagnosed_and_skipped() {
        let duplicate = Node3D::new(id("duplicate"));
        let missing = Node3D::new(id("missing-child")).parent(id("absent"));
        let cycle_a = Node3D::new(id("cycle-a")).parent(id("cycle-b"));
        let cycle_b = Node3D::new(id("cycle-b")).parent(id("cycle-a"));

        let ir = resolve_nodes(&[duplicate.clone(), duplicate, missing, cycle_a, cycle_b]);

        assert!(ir.nodes.is_empty());
        assert!(ir.diagnostics.iter().any(|diagnostic| matches!(
            diagnostic,
            Scene3DDiagnostic::DuplicateNodeId { id: node } if *node == id("duplicate")
        )));
        assert!(ir.diagnostics.iter().any(|diagnostic| matches!(
            diagnostic,
            Scene3DDiagnostic::MissingParent { id: node, .. } if *node == id("missing-child")
        )));
        assert!(ir
            .diagnostics
            .iter()
            .any(|diagnostic| matches!(diagnostic, Scene3DDiagnostic::ParentCycle { .. })));
    }

    #[test]
    fn invalid_quaternion_does_not_enter_closed_ir() {
        let invalid = Node3D::new(id("invalid")).transform(Transform3D {
            rotation: Rotation3D::new(0.0, 0.0, 0.0, 0.0),
            ..Transform3D::default()
        });

        let ir = resolve_nodes(&[invalid]);

        assert!(ir.nodes.is_empty());
        assert_eq!(
            ir.diagnostics,
            vec![Scene3DDiagnostic::InvalidTransform { id: id("invalid") }]
        );
    }

    #[test]
    fn invalid_mesh_indices_do_not_enter_closed_ir() {
        let invalid = Node3D::new(id("invalid-mesh")).primitive(Primitive3D::Mesh {
            vertices: vec![
                Point3D::new(0.0, 0.0, 0.0),
                Point3D::new(1.0, 0.0, 0.0),
                Point3D::new(0.0, 1.0, 0.0),
            ],
            indices: vec![0, 1, 4],
            color: fission_core::op::Color::BLUE,
        });

        let ir = resolve_nodes(&[invalid]);

        assert!(ir.nodes.is_empty());
        assert_eq!(
            ir.diagnostics,
            vec![Scene3DDiagnostic::InvalidPrimitive {
                id: id("invalid-mesh")
            }]
        );
    }
}
