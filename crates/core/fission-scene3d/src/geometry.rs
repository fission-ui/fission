use fission_scene::{Bounds3, Quat, Transform3, Vec3};

pub(crate) fn vec3(value: Vec3) -> glam::Vec3 {
    glam::Vec3::new(value.x, value.y, value.z)
}

pub(crate) fn value3(value: glam::Vec3) -> Vec3 {
    Vec3::new(value.x, value.y, value.z)
}

pub(crate) fn transform_matrix(transform: Transform3) -> Option<glam::Mat4> {
    if !transform.is_valid() {
        return None;
    }
    let Quat { x, y, z, w } = transform.rotation;
    let rotation = glam::Quat::from_xyzw(x, y, z, w).normalize();
    Some(glam::Mat4::from_scale_rotation_translation(
        vec3(transform.scale),
        rotation,
        vec3(transform.translation),
    ))
}

pub(crate) fn transform_bounds(bounds: Bounds3, matrix: glam::Mat4) -> Bounds3 {
    let min = vec3(bounds.min);
    let max = vec3(bounds.max);
    let mut world_min = glam::Vec3::splat(f32::INFINITY);
    let mut world_max = glam::Vec3::splat(f32::NEG_INFINITY);
    for x in [min.x, max.x] {
        for y in [min.y, max.y] {
            for z in [min.z, max.z] {
                let point = matrix.transform_point3(glam::Vec3::new(x, y, z));
                world_min = world_min.min(point);
                world_max = world_max.max(point);
            }
        }
    }
    Bounds3::new(value3(world_min), value3(world_max))
}

pub(crate) fn union_bounds(left: Bounds3, right: Bounds3) -> Bounds3 {
    Bounds3::new(
        value3(vec3(left.min).min(vec3(right.min))),
        value3(vec3(left.max).max(vec3(right.max))),
    )
}

pub(crate) fn bounds_from_positions(positions: &[Vec3]) -> Option<Bounds3> {
    let first = *positions.first()?;
    if !first.is_finite() {
        return None;
    }
    let mut min = vec3(first);
    let mut max = min;
    for position in &positions[1..] {
        if !position.is_finite() {
            return None;
        }
        min = min.min(vec3(*position));
        max = max.max(vec3(*position));
    }
    Some(Bounds3::new(value3(min), value3(max)))
}
