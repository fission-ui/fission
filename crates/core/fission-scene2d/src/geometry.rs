use fission_scene::{Bounds2, Transform2, Vec2};
use serde::{Deserialize, Serialize};

/// A finite axis-aligned rectangle in scene coordinates.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Rect2D {
    pub origin: Vec2,
    pub size: Vec2,
}

impl Rect2D {
    pub const fn new(origin: Vec2, size: Vec2) -> Self {
        Self { origin, size }
    }

    pub fn is_valid(self) -> bool {
        self.origin.is_finite() && self.size.is_finite() && self.size.x >= 0.0 && self.size.y >= 0.0
    }

    pub fn bounds(self) -> Bounds2 {
        Bounds2::new(
            self.origin,
            Vec2::new(self.origin.x + self.size.x, self.origin.y + self.size.y),
        )
    }

    pub fn contains(self, point: Vec2) -> bool {
        self.bounds().contains(point)
    }
}

/// Backend-neutral 2D affine transform.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Affine2 {
    pub m11: f32,
    pub m12: f32,
    pub m21: f32,
    pub m22: f32,
    pub tx: f32,
    pub ty: f32,
}

impl Affine2 {
    pub const IDENTITY: Self = Self {
        m11: 1.0,
        m12: 0.0,
        m21: 0.0,
        m22: 1.0,
        tx: 0.0,
        ty: 0.0,
    };

    pub fn from_transform(transform: Transform2) -> Self {
        let (sin, cos) = transform.rotation_radians.sin_cos();
        let m11 = cos * transform.scale.x;
        let m12 = sin * transform.scale.x;
        let m21 = -sin * transform.scale.y;
        let m22 = cos * transform.scale.y;
        let pivot = transform.pivot;
        Self {
            m11,
            m12,
            m21,
            m22,
            tx: transform.translation.x + pivot.x - m11 * pivot.x - m21 * pivot.y,
            ty: transform.translation.y + pivot.y - m12 * pivot.x - m22 * pivot.y,
        }
    }

    /// Compose `self` with `child`, applying `child` first.
    pub fn then(self, child: Self) -> Self {
        Self {
            m11: self.m11 * child.m11 + self.m21 * child.m12,
            m12: self.m12 * child.m11 + self.m22 * child.m12,
            m21: self.m11 * child.m21 + self.m21 * child.m22,
            m22: self.m12 * child.m21 + self.m22 * child.m22,
            tx: self.m11 * child.tx + self.m21 * child.ty + self.tx,
            ty: self.m12 * child.tx + self.m22 * child.ty + self.ty,
        }
    }

    pub fn transform_point(self, point: Vec2) -> Vec2 {
        Vec2::new(
            self.m11 * point.x + self.m21 * point.y + self.tx,
            self.m12 * point.x + self.m22 * point.y + self.ty,
        )
    }

    pub fn inverse(self) -> Option<Self> {
        let determinant = self.m11 * self.m22 - self.m21 * self.m12;
        if !determinant.is_finite() || determinant.abs() <= f32::EPSILON {
            return None;
        }
        let inverse = 1.0 / determinant;
        let m11 = self.m22 * inverse;
        let m12 = -self.m12 * inverse;
        let m21 = -self.m21 * inverse;
        let m22 = self.m11 * inverse;
        Some(Self {
            m11,
            m12,
            m21,
            m22,
            tx: -(m11 * self.tx + m21 * self.ty),
            ty: -(m12 * self.tx + m22 * self.ty),
        })
    }

    pub fn transform_bounds(self, bounds: Bounds2) -> Bounds2 {
        let corners = [
            bounds.min,
            Vec2::new(bounds.max.x, bounds.min.y),
            bounds.max,
            Vec2::new(bounds.min.x, bounds.max.y),
        ];
        let first = self.transform_point(corners[0]);
        let mut min = first;
        let mut max = first;
        for point in corners
            .into_iter()
            .skip(1)
            .map(|point| self.transform_point(point))
        {
            min.x = min.x.min(point.x);
            min.y = min.y.min(point.y);
            max.x = max.x.max(point.x);
            max.y = max.y.max(point.y);
        }
        Bounds2::new(min, max)
    }
}

impl Default for Affine2 {
    fn default() -> Self {
        Self::IDENTITY
    }
}
