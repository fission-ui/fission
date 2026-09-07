//! Backend-neutral ray picking for retained 3D scene nodes.

use crate::{Node3DId, Point3D, Primitive3D, Scene3D};

const INTERSECTION_EPSILON: f32 = 1.0e-6;

/// Normalized world-space ray used for retained scene queries.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ray3D {
    pub origin: Point3D,
    pub direction: Point3D,
}

impl Ray3D {
    /// Creates a ray when its origin and direction are finite and the direction
    /// has a non-zero length.
    pub fn new(origin: Point3D, direction: Point3D) -> Option<Self> {
        let origin = point_to_vec3(origin);
        let direction = point_to_vec3(direction);
        if !origin.is_finite()
            || !direction.is_finite()
            || direction.length_squared() <= f32::EPSILON
        {
            return None;
        }
        Some(Self {
            origin: vec3_to_point(origin),
            direction: vec3_to_point(direction.normalize()),
        })
    }
}

/// Closest retained scene node intersected by a world-space ray.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Scene3DHit {
    pub node: Node3DId,
    pub distance: f32,
    pub point: Point3D,
    pub normal: Point3D,
}

pub(crate) fn raycast(scene: &Scene3D, ray: Ray3D) -> Option<Scene3DHit> {
    let origin = point_to_vec3(ray.origin);
    let direction = point_to_vec3(ray.direction);
    let mut closest = None;

    for node in scene.render_nodes().iter().filter(|node| node.visible) {
        let Some(primitive) = &node.primitive else {
            continue;
        };
        let world = glam::Mat4::from_cols_array(&node.world_transform);
        let determinant = world.determinant();
        if !determinant.is_finite() || determinant.abs() <= INTERSECTION_EPSILON {
            continue;
        }
        let inverse = world.inverse();
        let local_origin = inverse.transform_point3(origin);
        let local_direction = inverse.transform_vector3(direction);
        let Some((distance, local_normal)) =
            intersect_primitive(local_origin, local_direction, primitive)
        else {
            continue;
        };
        if !distance.is_finite() || distance < 0.0 {
            continue;
        }
        let replace = closest
            .as_ref()
            .is_none_or(|hit: &Scene3DHit| distance < hit.distance);
        if replace {
            let normal_matrix = glam::Mat3::from_mat4(inverse.transpose());
            let world_normal = normal_matrix.mul_vec3(local_normal).normalize_or_zero();
            if world_normal.length_squared() <= f32::EPSILON {
                continue;
            }
            closest = Some(Scene3DHit {
                node: node.id,
                distance,
                point: vec3_to_point(origin + direction * distance),
                normal: vec3_to_point(world_normal),
            });
        }
    }

    closest
}

fn intersect_primitive(
    origin: glam::Vec3,
    direction: glam::Vec3,
    primitive: &Primitive3D,
) -> Option<(f32, glam::Vec3)> {
    match primitive {
        Primitive3D::Cube { center, size, .. } => {
            intersect_box(origin, direction, point_to_vec3(*center), *size * 0.5)
        }
        Primitive3D::Sphere { center, radius, .. } => {
            intersect_sphere(origin, direction, point_to_vec3(*center), *radius)
        }
        Primitive3D::Mesh {
            vertices, indices, ..
        } => intersect_mesh(origin, direction, vertices, indices),
    }
}

fn intersect_box(
    origin: glam::Vec3,
    direction: glam::Vec3,
    center: glam::Vec3,
    half_extent: f32,
) -> Option<(f32, glam::Vec3)> {
    let minimum = center - glam::Vec3::splat(half_extent);
    let maximum = center + glam::Vec3::splat(half_extent);
    let mut near = f32::NEG_INFINITY;
    let mut far = f32::INFINITY;
    let mut near_normal = glam::Vec3::ZERO;
    let mut far_normal = glam::Vec3::ZERO;

    for axis in 0..3 {
        let axis_direction = direction[axis];
        if axis_direction.abs() <= INTERSECTION_EPSILON {
            if origin[axis] < minimum[axis] || origin[axis] > maximum[axis] {
                return None;
            }
            continue;
        }
        let inverse_direction = 1.0 / axis_direction;
        let mut first = (minimum[axis] - origin[axis]) * inverse_direction;
        let mut second = (maximum[axis] - origin[axis]) * inverse_direction;
        let mut first_normal = axis_normal(axis, -1.0);
        let mut second_normal = axis_normal(axis, 1.0);
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

fn axis_normal(axis: usize, sign: f32) -> glam::Vec3 {
    let mut normal = glam::Vec3::ZERO;
    normal[axis] = sign;
    normal
}

fn intersect_sphere(
    origin: glam::Vec3,
    direction: glam::Vec3,
    center: glam::Vec3,
    radius: f32,
) -> Option<(f32, glam::Vec3)> {
    let offset = origin - center;
    let a = direction.length_squared();
    let half_b = offset.dot(direction);
    let c = offset.length_squared() - radius * radius;
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
        (distance, (point - center).normalize())
    })
}

fn intersect_mesh(
    origin: glam::Vec3,
    direction: glam::Vec3,
    vertices: &[Point3D],
    indices: &[u32],
) -> Option<(f32, glam::Vec3)> {
    let mut closest = None;
    for triangle in indices.chunks_exact(3) {
        let [a, b, c] = triangle else {
            continue;
        };
        let (Some(a), Some(b), Some(c)) = (
            vertices.get(*a as usize),
            vertices.get(*b as usize),
            vertices.get(*c as usize),
        ) else {
            continue;
        };
        let a = point_to_vec3(*a);
        let b = point_to_vec3(*b);
        let c = point_to_vec3(*c);
        let edge_ab = b - a;
        let edge_ac = c - a;
        let p = direction.cross(edge_ac);
        let determinant = edge_ab.dot(p);
        if determinant.abs() <= INTERSECTION_EPSILON {
            continue;
        }
        let inverse_determinant = 1.0 / determinant;
        let offset = origin - a;
        let u = offset.dot(p) * inverse_determinant;
        if !(0.0..=1.0).contains(&u) {
            continue;
        }
        let q = offset.cross(edge_ab);
        let v = direction.dot(q) * inverse_determinant;
        if v < 0.0 || u + v > 1.0 {
            continue;
        }
        let distance = edge_ac.dot(q) * inverse_determinant;
        if distance < 0.0 {
            continue;
        }
        let replace = closest
            .as_ref()
            .is_none_or(|(closest_distance, _)| distance < *closest_distance);
        if replace {
            let mut normal = edge_ab.cross(edge_ac).normalize_or_zero();
            if normal.dot(direction) > 0.0 {
                normal = -normal;
            }
            closest = Some((distance, normal));
        }
    }
    closest
}

fn point_to_vec3(point: Point3D) -> glam::Vec3 {
    glam::Vec3::new(point.x, point.y, point.z)
}

fn vec3_to_point(vector: glam::Vec3) -> Point3D {
    Point3D::new(vector.x, vector.y, vector.z)
}

#[cfg(test)]
mod tests {
    use fission_core::op::Color;

    use super::*;
    use crate::{Node3D, Transform3D};

    fn ray(origin: Point3D, direction: Point3D) -> Ray3D {
        Ray3D::new(origin, direction).expect("valid test ray")
    }

    #[test]
    fn closest_visible_transformed_node_wins() {
        let near = Node3DId::explicit("near");
        let far = Node3DId::explicit("far");
        let scene = Scene3D::new()
            .add_node(
                Node3D::new(far)
                    .transform(Transform3D::from_translation(Point3D::new(0.0, 0.0, -4.0)))
                    .primitive(Primitive3D::Cube {
                        center: Point3D::new(0.0, 0.0, 0.0),
                        size: 2.0,
                        color: Color::BLUE,
                    }),
            )
            .add_node(
                Node3D::new(near)
                    .transform(Transform3D::from_translation(Point3D::new(0.0, 0.0, 1.0)))
                    .primitive(Primitive3D::Sphere {
                        center: Point3D::new(0.0, 0.0, 0.0),
                        radius: 1.0,
                        color: Color::WHITE,
                    }),
            );

        let hit = raycast(
            &scene,
            ray(Point3D::new(0.0, 0.0, 5.0), Point3D::new(0.0, 0.0, -1.0)),
        )
        .expect("ray should hit the scene");

        assert_eq!(hit.node, near);
        assert!((hit.distance - 3.0).abs() < 1.0e-5);
        assert_eq!(hit.point, Point3D::new(0.0, 0.0, 2.0));
        assert_eq!(hit.normal, Point3D::new(0.0, 0.0, 1.0));
    }

    #[test]
    fn mesh_picking_reports_world_point_and_transformed_normal() {
        let node = Node3DId::explicit("triangle");
        let scene = Scene3D::new().add_node(
            Node3D::new(node)
                .transform(Transform3D {
                    translation: Point3D::new(2.0, 0.0, 0.0),
                    scale: Point3D::new(2.0, 1.0, 1.0),
                    ..Transform3D::default()
                })
                .primitive(Primitive3D::Mesh {
                    vertices: vec![
                        Point3D::new(-1.0, -1.0, 0.0),
                        Point3D::new(1.0, -1.0, 0.0),
                        Point3D::new(0.0, 1.0, 0.0),
                    ],
                    indices: vec![0, 1, 2],
                    color: Color::BLUE,
                }),
        );

        let hit = scene
            .raycast(ray(
                Point3D::new(2.0, 0.0, 4.0),
                Point3D::new(0.0, 0.0, -2.0),
            ))
            .expect("ray should hit transformed triangle");

        assert_eq!(hit.node, node);
        assert!((hit.distance - 4.0).abs() < 1.0e-5);
        assert_eq!(hit.point, Point3D::new(2.0, 0.0, 0.0));
        assert_eq!(hit.normal, Point3D::new(0.0, 0.0, 1.0));
    }

    #[test]
    fn hidden_and_singular_nodes_are_not_pickable() {
        let hidden = Node3D::new(Node3DId::explicit("hidden"))
            .visible(false)
            .primitive(Primitive3D::Sphere {
                center: Point3D::new(0.0, 0.0, 0.0),
                radius: 1.0,
                color: Color::BLUE,
            });
        let singular = Node3D::new(Node3DId::explicit("singular"))
            .transform(Transform3D {
                scale: Point3D::new(0.0, 1.0, 1.0),
                ..Transform3D::default()
            })
            .primitive(Primitive3D::Cube {
                center: Point3D::new(0.0, 0.0, 0.0),
                size: 2.0,
                color: Color::WHITE,
            });
        let scene = Scene3D::new().add_node(hidden).add_node(singular);

        assert!(scene
            .raycast(ray(
                Point3D::new(0.0, 0.0, 4.0),
                Point3D::new(0.0, 0.0, -1.0),
            ))
            .is_none());
    }

    #[test]
    fn invalid_rays_are_rejected() {
        assert!(Ray3D::new(Point3D::new(0.0, 0.0, 0.0), Point3D::new(0.0, 0.0, 0.0),).is_none());
        assert!(Ray3D::new(
            Point3D::new(f32::NAN, 0.0, 0.0),
            Point3D::new(0.0, 0.0, -1.0),
        )
        .is_none());
    }
}
