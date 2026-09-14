//! Column-major 4x4 transform matrices for compositor layers, applied to row vectors.

use super::*;

pub(super) fn composite_transform_matrix(
    rect: LayoutRect,
    translate_x: f32,
    translate_y: f32,
    scale: f32,
    rotation: f32,
) -> [f32; 16] {
    let center_x = rect.origin.x + rect.size.width * 0.5;
    let center_y = rect.origin.y + rect.size.height * 0.5;

    let to_center = translation_matrix(center_x, center_y);
    let from_center = translation_matrix(-center_x, -center_y);
    let scale_matrix = scale_matrix(scale);
    // Rotation tracks and composite rotations are in degrees.
    let rotation_matrix = rotation_z_matrix(rotation.to_radians());
    let motion_translate = translation_matrix(translate_x, translate_y);

    // Matrices use translation in indices 12/13 and are applied to points in
    // row-vector order. Compose operations in that same order so scale and
    // rotation preserve the widget's visual center.
    multiply_matrix(
        from_center,
        multiply_matrix(
            scale_matrix,
            multiply_matrix(
                rotation_matrix,
                multiply_matrix(to_center, motion_translate),
            ),
        ),
    )
}

pub(super) fn translation_matrix(tx: f32, ty: f32) -> [f32; 16] {
    [
        1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, tx, ty, 0.0, 1.0,
    ]
}

pub(super) fn scale_matrix(scale: f32) -> [f32; 16] {
    [
        scale, 0.0, 0.0, 0.0, 0.0, scale, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
    ]
}

pub(super) fn rotation_z_matrix(radians: f32) -> [f32; 16] {
    let sin = radians.sin();
    let cos = radians.cos();
    [
        cos, sin, 0.0, 0.0, -sin, cos, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
    ]
}

pub(super) fn multiply_matrix(a: [f32; 16], b: [f32; 16]) -> [f32; 16] {
    let mut out = [0.0; 16];
    for row in 0..4 {
        for col in 0..4 {
            let mut sum = 0.0;
            for k in 0..4 {
                sum += a[row * 4 + k] * b[k * 4 + col];
            }
            out[row * 4 + col] = sum;
        }
    }
    out
}

pub(super) fn is_identity_matrix(matrix: &[f32; 16]) -> bool {
    const IDENTITY: [f32; 16] = [
        1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
    ];
    matrix
        .iter()
        .zip(IDENTITY.iter())
        .all(|(lhs, rhs)| (*lhs - *rhs).abs() <= 0.000_1)
}
