//! Strongly typed two-dimensional simulation geometry.

use std::ops::{Add, AddAssign, Div, Mul, Sub, SubAssign};

use serde::{Deserialize, Serialize};

/// A distance in logical game-world pixels.
///
/// `Px` keeps world distances distinct from untyped scalar values while still
/// supporting the arithmetic needed by fixed-step simulation.
#[derive(Clone, Copy, Debug, Default, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Px(pub f32);

impl Px {
    pub const ZERO: Self = Self(0.0);

    pub const fn new(value: f32) -> Self {
        Self(value)
    }

    pub const fn get(self) -> f32 {
        self.0
    }
}

impl Add for Px {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self(self.0 + rhs.0)
    }
}

impl AddAssign for Px {
    fn add_assign(&mut self, rhs: Self) {
        self.0 += rhs.0;
    }
}

impl Sub for Px {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        Self(self.0 - rhs.0)
    }
}

impl SubAssign for Px {
    fn sub_assign(&mut self, rhs: Self) {
        self.0 -= rhs.0;
    }
}

impl Mul<f32> for Px {
    type Output = Self;

    fn mul(self, rhs: f32) -> Self::Output {
        Self(self.0 * rhs)
    }
}

impl Div<f32> for Px {
    type Output = Self;

    fn div(self, rhs: f32) -> Self::Output {
        Self(self.0 / rhs)
    }
}

/// A point in a two-dimensional game world.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Place {
    pub x: Px,
    pub y: Px,
}

impl Place {
    pub const fn new(x: Px, y: Px) -> Self {
        Self { x, y }
    }
}

/// A two-dimensional extent in logical game-world pixels.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Size {
    pub width: Px,
    pub height: Px,
}

impl Size {
    pub const fn new(width: Px, height: Px) -> Self {
        Self { width, height }
    }

    /// Returns whether both dimensions are finite and non-negative.
    pub fn is_valid(self) -> bool {
        self.width.0.is_finite()
            && self.height.0.is_finite()
            && self.width.0 >= 0.0
            && self.height.0 >= 0.0
    }
}

/// An axis-aligned world-space rectangle.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Bounds2D {
    pub min: Place,
    pub max: Place,
}

impl Bounds2D {
    pub fn from_top_left(top_left: Place, size: Size) -> Self {
        Self {
            min: top_left,
            max: Place::new(top_left.x + size.width, top_left.y + size.height),
        }
    }

    pub fn from_center(center: Place, size: Size) -> Self {
        let half_width = size.width / 2.0;
        let half_height = size.height / 2.0;
        Self {
            min: Place::new(center.x - half_width, center.y - half_height),
            max: Place::new(center.x + half_width, center.y + half_height),
        }
    }

    pub fn width(self) -> Px {
        self.max.x - self.min.x
    }

    pub fn height(self) -> Px {
        self.max.y - self.min.y
    }

    pub fn center(self) -> Place {
        Place::new(
            self.min.x + self.width() / 2.0,
            self.min.y + self.height() / 2.0,
        )
    }

    pub fn contains(self, point: Place) -> bool {
        point.x >= self.min.x
            && point.x <= self.max.x
            && point.y >= self.min.y
            && point.y <= self.max.y
    }

    pub fn contains_bounds(self, other: Self) -> bool {
        self.contains(other.min) && self.contains(other.max)
    }

    /// Reports overlap inclusively, so touching edges count as contact.
    pub fn overlaps(self, other: Self) -> bool {
        self.min.x <= other.max.x
            && self.max.x >= other.min.x
            && self.min.y <= other.max.y
            && self.max.y >= other.min.y
    }

    pub fn is_valid(self) -> bool {
        self.min.x.0.is_finite()
            && self.min.y.0.is_finite()
            && self.max.x.0.is_finite()
            && self.max.y.0.is_finite()
            && self.min.x <= self.max.x
            && self.min.y <= self.max.y
    }
}

/// A clockwise rotation expressed in degrees.
#[derive(Clone, Copy, Debug, Default, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Degrees(pub f32);

impl Degrees {
    pub const fn new(value: f32) -> Self {
        Self(value)
    }
}

/// A world-space speed measured in logical pixels per second.
#[derive(Clone, Copy, Debug, Default, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(transparent)]
pub struct PxPerSecond(pub f32);

impl PxPerSecond {
    pub const fn new(value: f32) -> Self {
        Self(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bounds_are_inclusive_and_construct_consistently() {
        let size = Size::new(Px(20.0), Px(10.0));
        let centered = Bounds2D::from_center(Place::new(Px(15.0), Px(10.0)), size);
        let top_left = Bounds2D::from_top_left(Place::new(Px(5.0), Px(5.0)), size);

        assert_eq!(centered, top_left);
        assert!(centered.contains(Place::new(Px(25.0), Px(15.0))));
        assert!(centered.overlaps(Bounds2D::from_top_left(
            Place::new(Px(25.0), Px(15.0)),
            Size::new(Px(4.0), Px(4.0)),
        )));
    }
}
