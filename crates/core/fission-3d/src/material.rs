//! Compact material declarations for retained 3D scene nodes.

use fission_core::op::Color;
use serde::{Deserialize, Serialize};

/// Surface response applied to one retained scene node.
///
/// The tint multiplies the primitive's authored color. Roughness and metallic
/// use the conventional zero-to-one range; emissive intensity is unbounded but
/// must remain finite and nonnegative.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Material3D {
    pub tint: Color,
    pub roughness: f32,
    pub metallic: f32,
    pub emissive_intensity: f32,
}

impl Material3D {
    pub const fn new(tint: Color, roughness: f32, metallic: f32) -> Self {
        Self {
            tint,
            roughness,
            metallic,
            emissive_intensity: 0.0,
        }
    }

    pub const fn emissive(mut self, intensity: f32) -> Self {
        self.emissive_intensity = intensity;
        self
    }

    pub(crate) fn is_valid(self) -> bool {
        self.roughness.is_finite()
            && (0.0..=1.0).contains(&self.roughness)
            && self.metallic.is_finite()
            && (0.0..=1.0).contains(&self.metallic)
            && self.emissive_intensity.is_finite()
            && self.emissive_intensity >= 0.0
    }
}

impl Default for Material3D {
    fn default() -> Self {
        Self::new(Color::WHITE, 0.62, 0.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn material_validation_rejects_invalid_surface_values() {
        assert!(Material3D::default().is_valid());
        assert!(!Material3D {
            roughness: 1.1,
            ..Material3D::default()
        }
        .is_valid());
        assert!(!Material3D {
            metallic: f32::NAN,
            ..Material3D::default()
        }
        .is_valid());
        assert!(!Material3D::default().emissive(-0.1).is_valid());
    }
}
