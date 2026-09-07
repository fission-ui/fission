//! Backend-neutral lighting declarations for retained 3D scenes.

use fission_core::op::Color;
use serde::{Deserialize, Serialize};

use crate::Point3D;

/// One world-space directional light. `direction` points toward the light.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct DirectionalLight3D {
    pub direction: Point3D,
    pub color: Color,
    pub intensity: f32,
}

impl DirectionalLight3D {
    pub const fn new(direction: Point3D, color: Color, intensity: f32) -> Self {
        Self {
            direction,
            color,
            intensity,
        }
    }

    pub(crate) fn is_valid(self) -> bool {
        let length_squared = self.direction.x * self.direction.x
            + self.direction.y * self.direction.y
            + self.direction.z * self.direction.z;
        self.direction.x.is_finite()
            && self.direction.y.is_finite()
            && self.direction.z.is_finite()
            && length_squared > f32::EPSILON
            && self.intensity.is_finite()
            && self.intensity >= 0.0
    }
}

impl Default for DirectionalLight3D {
    fn default() -> Self {
        Self::new(
            Point3D::new(0.45, 0.8, 0.35),
            Color {
                r: 255,
                g: 250,
                b: 235,
                a: 255,
            },
            0.82,
        )
    }
}

/// Compact lighting environment used by Fission's built-in 3D renderer.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct SceneLighting3D {
    pub ambient_intensity: f32,
    pub directional: DirectionalLight3D,
    pub specular_intensity: f32,
    pub specular_exponent: f32,
}

impl SceneLighting3D {
    pub(crate) fn is_valid(self) -> bool {
        self.directional.is_valid()
            && self.ambient_intensity.is_finite()
            && self.ambient_intensity >= 0.0
            && self.specular_intensity.is_finite()
            && self.specular_intensity >= 0.0
            && self.specular_exponent.is_finite()
            && self.specular_exponent >= 1.0
    }
}

impl Default for SceneLighting3D {
    fn default() -> Self {
        Self {
            ambient_intensity: 0.24,
            directional: DirectionalLight3D::default(),
            specular_intensity: 0.22,
            specular_exponent: 32.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lighting_rejects_degenerate_or_non_finite_values() {
        assert!(SceneLighting3D::default().is_valid());
        assert!(!SceneLighting3D {
            directional: DirectionalLight3D::new(Point3D::new(0.0, 0.0, 0.0), Color::WHITE, 1.0),
            ..SceneLighting3D::default()
        }
        .is_valid());
        assert!(!SceneLighting3D {
            ambient_intensity: f32::NAN,
            ..SceneLighting3D::default()
        }
        .is_valid());
        assert!(!SceneLighting3D {
            specular_exponent: 0.5,
            ..SceneLighting3D::default()
        }
        .is_valid());
    }
}
