use fission_scene::{Bounds3, NodeId, Rgba, SceneId, Vec2, Vec3};
use fission_scene3d::{
    Material3D, Mesh3D, MeshId, MeshVertex3D, Node3D, NodeContent3D, Primitive3D as ScenePrimitive,
    ResourceId, Scene3DIR, Scene3DRenderPacket, Viewport3D,
};

use crate::Primitive3D;

/// Decodes the payload emitted by Fission 0.15's original primitive widget and
/// translates it into the closed scene packet consumed by the current renderer.
pub fn decode_legacy_render_packet(
    payload: &[u8],
    width: f32,
    height: f32,
) -> Result<Scene3DRenderPacket, String> {
    let primitives = bincode::deserialize::<Vec<Primitive3D>>(payload)
        .map_err(|error| format!("legacy 3D payload is invalid: {error}"))?;
    let mut scene = Scene3DIR::new(
        SceneId(stable_payload_id(payload)),
        Viewport3D::new(width, height),
    );
    for (index, primitive) in primitives.into_iter().enumerate() {
        let node_id = NodeId(index as u64 + 1);
        let material_id = ResourceId(index as u64 + 1);
        let (content, color) = match primitive {
            Primitive3D::Cube {
                center,
                size,
                color,
            } => {
                let mut node = Node3D::group(node_id);
                node.transform.translation = Vec3::new(center.x, center.y, center.z);
                scene.nodes.push(Node3D {
                    content: NodeContent3D::Primitive(ScenePrimitive::Cube {
                        size: Vec3::new(size, size, size),
                    }),
                    material: Some(material_id),
                    ..node
                });
                continue_with_material(&mut scene, material_id, color);
                continue;
            }
            Primitive3D::Sphere {
                center,
                radius,
                color,
            } => {
                let mut node = Node3D::group(node_id);
                node.transform.translation = Vec3::new(center.x, center.y, center.z);
                scene.nodes.push(Node3D {
                    content: NodeContent3D::Primitive(ScenePrimitive::Sphere { radius }),
                    material: Some(material_id),
                    ..node
                });
                continue_with_material(&mut scene, material_id, color);
                continue;
            }
            Primitive3D::Mesh {
                vertices,
                indices,
                color,
            } => {
                let mesh_id = MeshId(index as u64 + 1);
                let positions = vertices
                    .into_iter()
                    .map(|vertex| Vec3::new(vertex.x, vertex.y, vertex.z))
                    .collect::<Vec<_>>();
                let bounds = bounds(&positions)
                    .ok_or_else(|| format!("legacy mesh {} has no finite vertices", index))?;
                let normals = smooth_normals(&positions, &indices);
                scene.resources.meshes.insert(
                    mesh_id,
                    Mesh3D {
                        revision: 0,
                        asset: None,
                        vertices: positions
                            .into_iter()
                            .zip(normals)
                            .map(|(position, normal)| MeshVertex3D {
                                position,
                                normal,
                                uv: Vec2::ZERO,
                            })
                            .collect(),
                        indices,
                        bounds,
                    },
                );
                (
                    NodeContent3D::Primitive(ScenePrimitive::Mesh { mesh: mesh_id }),
                    color,
                )
            }
        };
        scene.nodes.push(Node3D {
            content,
            material: Some(material_id),
            ..Node3D::group(node_id)
        });
        continue_with_material(&mut scene, material_id, color);
    }
    Ok(Scene3DRenderPacket::new(scene))
}

fn stable_payload_id(payload: &[u8]) -> u64 {
    // FNV-1a keeps legacy cache identity deterministic without making the
    // compatibility path authoritative for modern scene identity.
    payload.iter().fold(0xcbf29ce484222325, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(0x100000001b3)
    })
}

fn continue_with_material(scene: &mut Scene3DIR, id: ResourceId, color: fission_core::op::Color) {
    scene.resources.materials.insert(
        id,
        Material3D {
            base_color: Rgba::new(
                color.r as f32 / 255.0,
                color.g as f32 / 255.0,
                color.b as f32 / 255.0,
                color.a as f32 / 255.0,
            ),
            ..Material3D::default()
        },
    );
}

fn bounds(vertices: &[Vec3]) -> Option<Bounds3> {
    let first = *vertices.first()?;
    if !first.is_finite() {
        return None;
    }
    let mut min = first;
    let mut max = first;
    for vertex in &vertices[1..] {
        if !vertex.is_finite() {
            return None;
        }
        min.x = min.x.min(vertex.x);
        min.y = min.y.min(vertex.y);
        min.z = min.z.min(vertex.z);
        max.x = max.x.max(vertex.x);
        max.y = max.y.max(vertex.y);
        max.z = max.z.max(vertex.z);
    }
    Some(Bounds3::new(min, max))
}

fn smooth_normals(vertices: &[Vec3], indices: &[u32]) -> Vec<Vec3> {
    let mut normals = vec![glam::Vec3::ZERO; vertices.len()];
    for triangle in indices.chunks_exact(3) {
        let [a, b, c] = [
            triangle[0] as usize,
            triangle[1] as usize,
            triangle[2] as usize,
        ];
        let Some((a_pos, b_pos, c_pos)) = vertices
            .get(a)
            .zip(vertices.get(b))
            .zip(vertices.get(c))
            .map(|((a, b), c)| (*a, *b, *c))
        else {
            continue;
        };
        let a_vec = glam::Vec3::new(a_pos.x, a_pos.y, a_pos.z);
        let b_vec = glam::Vec3::new(b_pos.x, b_pos.y, b_pos.z);
        let c_vec = glam::Vec3::new(c_pos.x, c_pos.y, c_pos.z);
        let normal = (b_vec - a_vec).cross(c_vec - a_vec);
        for index in [a, b, c] {
            normals[index] += normal;
        }
    }
    normals
        .into_iter()
        .map(|normal| {
            let normal = normal.try_normalize().unwrap_or(glam::Vec3::Y);
            Vec3::new(normal.x, normal.y, normal.z)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Point3D, Primitive3D};

    #[test]
    fn legacy_payload_becomes_typed_scene_packet() {
        let payload = bincode::serialize(&vec![Primitive3D::Cube {
            center: Point3D::new(1.0, 2.0, 3.0),
            size: 2.0,
            color: fission_core::op::Color {
                r: 10,
                g: 20,
                b: 30,
                a: 255,
            },
        }])
        .unwrap();
        let packet = decode_legacy_render_packet(&payload, 320.0, 180.0).unwrap();
        assert_eq!(packet.prepared.draws.len(), 1);
        assert_eq!(packet.prepared.source.resources.materials.len(), 1);
    }
}
