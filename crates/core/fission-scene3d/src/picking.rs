use fission_scene::{NodeId, Vec2, Vec3};

use crate::geometry::{value3, vec3};
use crate::{CameraProjection3D, NodeContent3D, PreparedScene3D, Primitive3D};

const EPSILON: f32 = 1.0e-6;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ray3D {
    pub origin: Vec3,
    pub direction: Vec3,
}

impl Ray3D {
    pub fn new(origin: Vec3, direction: Vec3) -> Option<Self> {
        let direction = vec3(direction);
        if !origin.is_finite()
            || !direction.is_finite()
            || direction.length_squared() <= f32::EPSILON
        {
            return None;
        }
        Some(Self {
            origin,
            direction: value3(direction.normalize()),
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PickHit3D {
    pub node: NodeId,
    pub distance: f32,
    pub position: Vec3,
    pub normal: Vec3,
}

impl PreparedScene3D {
    pub fn viewport_ray(&self, point: Vec2) -> Option<Ray3D> {
        let viewport = self.source.viewport;
        if !viewport.is_valid() || !point.is_finite() {
            return None;
        }
        let local = Vec2::new(point.x - viewport.origin.x, point.y - viewport.origin.y);
        if local.x < 0.0 || local.y < 0.0 || local.x > viewport.size.x || local.y > viewport.size.y
        {
            return None;
        }
        let camera = self.source.camera;
        if !camera.is_valid() {
            return None;
        }
        let eye = vec3(camera.eye);
        let view = glam::Mat4::look_at_rh(eye, vec3(camera.target), vec3(camera.up).normalize());
        let aspect = viewport.size.x / viewport.size.y;
        let projection = match camera.projection {
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
        let inverse = (projection * view).inverse();
        if !inverse.is_finite() {
            return None;
        }
        let ndc_x = local.x * 2.0 / viewport.size.x - 1.0;
        let ndc_y = 1.0 - local.y * 2.0 / viewport.size.y;
        let unproject = |depth| {
            let point = inverse * glam::Vec4::new(ndc_x, ndc_y, depth, 1.0);
            (point.w.abs() > EPSILON).then(|| point.truncate() / point.w)
        };
        let near = unproject(0.0)?;
        let far = unproject(1.0)?;
        let origin = if matches!(camera.projection, CameraProjection3D::Perspective { .. }) {
            eye
        } else {
            near
        };
        Ray3D::new(value3(origin), value3(far - origin))
    }

    pub fn pick_viewport(&self, point: Vec2) -> Option<PickHit3D> {
        self.viewport_ray(point).and_then(|ray| self.raycast(ray))
    }

    pub fn raycast(&self, ray: Ray3D) -> Option<PickHit3D> {
        let origin = vec3(ray.origin);
        let direction = vec3(ray.direction);
        let mut closest = None;
        for node in self.nodes.iter().filter(|node| node.pickable) {
            let world = glam::Mat4::from_cols_array(&node.world_transform);
            let determinant = world.determinant();
            if !determinant.is_finite() || determinant.abs() <= EPSILON {
                continue;
            }
            let inverse = world.inverse();
            let local_origin = inverse.transform_point3(origin);
            let local_direction = inverse.transform_vector3(direction);
            let intersection = match node.content {
                NodeContent3D::Group => None,
                NodeContent3D::Primitive(Primitive3D::Cube { size }) => intersect_box(
                    local_origin,
                    local_direction,
                    -vec3(size) * 0.5,
                    vec3(size) * 0.5,
                ),
                NodeContent3D::Primitive(Primitive3D::Sphere { radius }) => {
                    intersect_sphere(local_origin, local_direction, radius)
                }
                NodeContent3D::Primitive(Primitive3D::Mesh { mesh }) => self
                    .source
                    .resources
                    .meshes
                    .get(&mesh)
                    .and_then(|mesh| intersect_mesh(local_origin, local_direction, mesh)),
                NodeContent3D::Model { .. } => intersect_box(
                    origin,
                    direction,
                    vec3(node.world_bounds.min),
                    vec3(node.world_bounds.max),
                ),
            };
            let Some((local_distance, local_normal)) = intersection else {
                continue;
            };
            let local_point = local_origin + local_direction * local_distance;
            let world_point = if matches!(node.content, NodeContent3D::Model { .. }) {
                local_point
            } else {
                world.transform_point3(local_point)
            };
            let distance = world_point.distance(origin);
            if closest
                .as_ref()
                .is_some_and(|hit: &PickHit3D| hit.distance <= distance)
            {
                continue;
            }
            let world_normal = if matches!(node.content, NodeContent3D::Model { .. }) {
                local_normal
            } else {
                glam::Mat3::from_mat4(inverse.transpose())
                    .mul_vec3(local_normal)
                    .normalize_or_zero()
            };
            closest = Some(PickHit3D {
                node: node.id,
                distance,
                position: value3(world_point),
                normal: value3(world_normal),
            });
        }
        closest
    }
}

fn intersect_box(
    origin: glam::Vec3,
    direction: glam::Vec3,
    min: glam::Vec3,
    max: glam::Vec3,
) -> Option<(f32, glam::Vec3)> {
    let mut near = f32::NEG_INFINITY;
    let mut far = f32::INFINITY;
    let mut near_normal = glam::Vec3::ZERO;
    let mut far_normal = glam::Vec3::ZERO;
    for axis in 0..3 {
        if direction[axis].abs() <= EPSILON {
            if origin[axis] < min[axis] || origin[axis] > max[axis] {
                return None;
            }
            continue;
        }
        let mut first = (min[axis] - origin[axis]) / direction[axis];
        let mut second = (max[axis] - origin[axis]) / direction[axis];
        let mut first_normal = glam::Vec3::ZERO;
        let mut second_normal = glam::Vec3::ZERO;
        first_normal[axis] = -1.0;
        second_normal[axis] = 1.0;
        if first > second {
            std::mem::swap(&mut first, &mut second);
            std::mem::swap(&mut first_normal, &mut second_normal);
        }
        if first > near {
            near = first;
            near_normal = first_normal;
        }
        if second < far {
            far = second;
            far_normal = second_normal;
        }
        if near > far {
            return None;
        }
    }
    if near >= 0.0 {
        Some((near, near_normal))
    } else if far >= 0.0 {
        Some((far, far_normal))
    } else {
        None
    }
}

fn intersect_sphere(
    origin: glam::Vec3,
    direction: glam::Vec3,
    radius: f32,
) -> Option<(f32, glam::Vec3)> {
    let a = direction.length_squared();
    let half_b = origin.dot(direction);
    let c = origin.length_squared() - radius * radius;
    let discriminant = half_b * half_b - a * c;
    if discriminant < 0.0 || a <= f32::EPSILON {
        return None;
    }
    let root = discriminant.sqrt();
    let near = (-half_b - root) / a;
    let far = (-half_b + root) / a;
    let distance = if near >= 0.0 { near } else { far };
    (distance >= 0.0).then(|| {
        let point = origin + direction * distance;
        (distance, point.normalize())
    })
}

fn intersect_mesh(
    origin: glam::Vec3,
    direction: glam::Vec3,
    mesh: &crate::Mesh3D,
) -> Option<(f32, glam::Vec3)> {
    let mut closest = None;
    for triangle in mesh.indices.chunks_exact(3) {
        let a = vec3(mesh.vertices[triangle[0] as usize].position);
        let b = vec3(mesh.vertices[triangle[1] as usize].position);
        let c = vec3(mesh.vertices[triangle[2] as usize].position);
        let edge_ab = b - a;
        let edge_ac = c - a;
        let p = direction.cross(edge_ac);
        let determinant = edge_ab.dot(p);
        if determinant.abs() <= EPSILON {
            continue;
        }
        let inverse_determinant = 1.0 / determinant;
        let offset = origin - a;
        let u = offset.dot(p) * inverse_determinant;
        let q = offset.cross(edge_ab);
        let v = direction.dot(q) * inverse_determinant;
        let distance = edge_ac.dot(q) * inverse_determinant;
        if u < 0.0 || v < 0.0 || u + v > 1.0 || distance < 0.0 {
            continue;
        }
        if closest
            .as_ref()
            .is_none_or(|(current, _): &(f32, glam::Vec3)| distance < *current)
        {
            let mut normal = edge_ab.cross(edge_ac).normalize_or_zero();
            if normal.dot(direction) > 0.0 {
                normal = -normal;
            }
            closest = Some((distance, normal));
        }
    }
    closest
}

#[cfg(test)]
mod tests {
    use fission_scene::{NodeId, SceneId, Transform3};

    use super::*;
    use crate::{
        Node3D, Primitive3D, RenderCapabilities3D, Scene3DIR, Scene3DProcessor, Viewport3D,
    };

    #[test]
    fn viewport_center_picks_closest_stable_node() {
        let mut scene = Scene3DIR::new(SceneId(1), Viewport3D::new(800.0, 600.0));
        scene.camera = crate::Camera3D::perspective(
            Vec3::new(0.0, 0.0, 5.0),
            Vec3::ZERO,
            60.0_f32.to_radians(),
        );
        scene.nodes.push(Node3D {
            id: NodeId(7),
            parent: None,
            transform: Transform3::IDENTITY,
            visible: true,
            content: NodeContent3D::Primitive(Primitive3D::Cube { size: Vec3::ONE }),
            material: None,
            blend_order: 0,
            pickable: true,
        });
        let prepared = Scene3DProcessor::new().prepare(&scene, RenderCapabilities3D::default());
        let hit = prepared
            .pick_viewport(Vec2::new(400.0, 300.0))
            .expect("center cube should be pickable");
        assert_eq!(hit.node, NodeId(7));
        assert!((hit.position.z - 0.5).abs() < 1.0e-4);
    }
}
