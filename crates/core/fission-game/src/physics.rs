//! Backend-neutral physics declarations shared by optional providers.

use serde::{Deserialize, Serialize};

use crate::{StableKey, StableKeyValue};

/// Stable application-owned identity for one physical body.
///
/// Provider handles are deliberately not exposed because they are transient and
/// cannot be used as save, replay, or network identities.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct PhysicsBodyId(pub StableKeyValue);

impl PhysicsBodyId {
    pub fn from_key(key: &impl StableKey) -> Self {
        Self(key.stable_key())
    }
}

/// A vector in physics-world units. Distances are conventionally metres.
///
/// The physics coordinate system is x-right and y-up. Presentation adapters
/// are responsible for mapping it into renderer coordinates such as a
/// top-left-origin 2D scene.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct PhysicsVector2 {
    pub x: f32,
    pub y: f32,
}

impl PhysicsVector2 {
    pub const ZERO: Self = Self::new(0.0, 0.0);

    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    pub fn is_finite(self) -> bool {
        self.x.is_finite() && self.y.is_finite()
    }
}

/// Translation and counter-clockwise rotation in radians.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct PhysicsPose2D {
    pub translation: PhysicsVector2,
    pub rotation: f32,
}

impl PhysicsPose2D {
    pub const fn new(translation: PhysicsVector2, rotation: f32) -> Self {
        Self {
            translation,
            rotation,
        }
    }

    pub fn is_finite(self) -> bool {
        self.translation.is_finite() && self.rotation.is_finite()
    }
}

/// Linear world velocity and counter-clockwise angular velocity in radians per second.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct PhysicsVelocity2D {
    pub linear: PhysicsVector2,
    pub angular: f32,
}

impl PhysicsVelocity2D {
    pub const fn new(linear: PhysicsVector2, angular: f32) -> Self {
        Self { linear, angular }
    }

    pub fn is_finite(self) -> bool {
        self.linear.is_finite() && self.angular.is_finite()
    }
}

/// Closest 2D physics body intersected by a world-space ray.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PhysicsRayHit2D {
    pub body: PhysicsBodyId,
    pub distance: f32,
    pub point: PhysicsVector2,
    pub normal: PhysicsVector2,
}

/// How a physics provider advances a body.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PhysicsBodyKind {
    /// Moved by forces, impulses, gravity, and contacts.
    #[default]
    Dynamic,
    /// Immovable world geometry.
    Fixed,
    /// Moved explicitly by game code while producing contact velocity.
    KinematicPosition,
    /// Moved from a velocity supplied by game code.
    KinematicVelocity,
}

/// Closed initial set of two-dimensional collider geometry.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "shape")]
pub enum PhysicsShape2D {
    Circle { radius: f32 },
    Cuboid { half_extents: PhysicsVector2 },
    CapsuleY { half_height: f32, radius: f32 },
}

/// A collider attached to a physical body.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Collider2D {
    pub shape: PhysicsShape2D,
    pub offset: PhysicsPose2D,
    pub density: f32,
    pub friction: f32,
    pub restitution: f32,
    pub sensor: bool,
}

impl Collider2D {
    pub fn new(shape: PhysicsShape2D) -> Self {
        Self {
            shape,
            offset: PhysicsPose2D::default(),
            density: 1.0,
            friction: 0.5,
            restitution: 0.0,
            sensor: false,
        }
    }
}

/// Complete provider-neutral declaration for one two-dimensional body.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PhysicsBody2D {
    pub id: PhysicsBodyId,
    pub kind: PhysicsBodyKind,
    pub pose: PhysicsPose2D,
    pub velocity: PhysicsVelocity2D,
    pub gravity_scale: f32,
    pub linear_damping: f32,
    pub angular_damping: f32,
    pub continuous_collision_detection: bool,
    pub colliders: Vec<Collider2D>,
}

impl PhysicsBody2D {
    pub fn dynamic(id: PhysicsBodyId, shape: PhysicsShape2D) -> Self {
        Self {
            id,
            kind: PhysicsBodyKind::Dynamic,
            pose: PhysicsPose2D::default(),
            velocity: PhysicsVelocity2D::default(),
            gravity_scale: 1.0,
            linear_damping: 0.0,
            angular_damping: 0.0,
            continuous_collision_detection: false,
            colliders: vec![Collider2D::new(shape)],
        }
    }

    pub fn fixed(id: PhysicsBodyId, shape: PhysicsShape2D) -> Self {
        Self {
            kind: PhysicsBodyKind::Fixed,
            ..Self::dynamic(id, shape)
        }
    }
}

/// Backend contract used by game code that opts into two-dimensional physics.
///
/// Applications can remain generic over this trait while selecting Rapier or a
/// future provider at their composition boundary.
pub trait PhysicsProvider2D {
    type Error: std::error::Error + Send + Sync + 'static;

    fn insert_body(&mut self, body: PhysicsBody2D) -> Result<(), Self::Error>;
    fn remove_body(&mut self, id: &PhysicsBodyId) -> bool;
    fn contains_body(&self, id: &PhysicsBodyId) -> bool;
    fn body_pose(&self, id: &PhysicsBodyId) -> Option<PhysicsPose2D>;
    fn body_velocity(&self, id: &PhysicsBodyId) -> Option<PhysicsVelocity2D>;
    fn set_body_pose(
        &mut self,
        id: &PhysicsBodyId,
        pose: PhysicsPose2D,
        wake_up: bool,
    ) -> Result<(), Self::Error>;
    fn set_body_velocity(
        &mut self,
        id: &PhysicsBodyId,
        velocity: PhysicsVelocity2D,
        wake_up: bool,
    ) -> Result<(), Self::Error>;
    fn add_force(
        &mut self,
        id: &PhysicsBodyId,
        force: PhysicsVector2,
        wake_up: bool,
    ) -> Result<(), Self::Error>;
    fn add_force_at_point(
        &mut self,
        id: &PhysicsBodyId,
        force: PhysicsVector2,
        point: PhysicsVector2,
        wake_up: bool,
    ) -> Result<(), Self::Error>;
    fn apply_impulse(
        &mut self,
        id: &PhysicsBodyId,
        impulse: PhysicsVector2,
        wake_up: bool,
    ) -> Result<(), Self::Error>;
    fn cast_ray(
        &self,
        origin: PhysicsVector2,
        direction: PhysicsVector2,
        max_distance: f32,
        solid: bool,
    ) -> Result<Option<PhysicsRayHit2D>, Self::Error>;
    fn step(&mut self, duration: crate::StepDuration);
}

/// A vector in right-handed physics-world units, conventionally metres.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct PhysicsVector3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl PhysicsVector3 {
    pub const ZERO: Self = Self::new(0.0, 0.0, 0.0);

    pub const fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }

    pub fn is_finite(self) -> bool {
        self.x.is_finite() && self.y.is_finite() && self.z.is_finite()
    }
}

/// Provider-neutral quaternion stored in `(x, y, z, w)` order.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct PhysicsRotation3D {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub w: f32,
}

impl PhysicsRotation3D {
    pub const IDENTITY: Self = Self::new(0.0, 0.0, 0.0, 1.0);

    pub const fn new(x: f32, y: f32, z: f32, w: f32) -> Self {
        Self { x, y, z, w }
    }

    pub fn is_valid(self) -> bool {
        self.x.is_finite()
            && self.y.is_finite()
            && self.z.is_finite()
            && self.w.is_finite()
            && self.length_squared() > f32::EPSILON
    }

    pub fn length_squared(self) -> f32 {
        self.x * self.x + self.y * self.y + self.z * self.z + self.w * self.w
    }
}

impl Default for PhysicsRotation3D {
    fn default() -> Self {
        Self::IDENTITY
    }
}

/// Translation and orientation of a three-dimensional physical body.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct PhysicsPose3D {
    pub translation: PhysicsVector3,
    pub rotation: PhysicsRotation3D,
}

impl PhysicsPose3D {
    pub const fn new(translation: PhysicsVector3, rotation: PhysicsRotation3D) -> Self {
        Self {
            translation,
            rotation,
        }
    }

    pub fn is_valid(self) -> bool {
        self.translation.is_finite() && self.rotation.is_valid()
    }
}

/// Linear and angular velocity in three-dimensional physics-world units.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct PhysicsVelocity3D {
    pub linear: PhysicsVector3,
    /// Axis-angle velocity in radians per second.
    pub angular: PhysicsVector3,
}

impl PhysicsVelocity3D {
    pub const fn new(linear: PhysicsVector3, angular: PhysicsVector3) -> Self {
        Self { linear, angular }
    }

    pub fn is_finite(self) -> bool {
        self.linear.is_finite() && self.angular.is_finite()
    }
}

/// Closest 3D physics body intersected by a world-space ray.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PhysicsRayHit3D {
    pub body: PhysicsBodyId,
    pub distance: f32,
    pub point: PhysicsVector3,
    pub normal: PhysicsVector3,
}

/// Closed initial set of three-dimensional collider geometry.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "shape")]
pub enum PhysicsShape3D {
    Sphere { radius: f32 },
    Cuboid { half_extents: PhysicsVector3 },
    CapsuleY { half_height: f32, radius: f32 },
}

/// A collider attached to a three-dimensional physical body.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Collider3D {
    pub shape: PhysicsShape3D,
    pub offset: PhysicsPose3D,
    pub density: f32,
    pub friction: f32,
    pub restitution: f32,
    pub sensor: bool,
}

impl Collider3D {
    pub fn new(shape: PhysicsShape3D) -> Self {
        Self {
            shape,
            offset: PhysicsPose3D::default(),
            density: 1.0,
            friction: 0.5,
            restitution: 0.0,
            sensor: false,
        }
    }
}

/// Complete provider-neutral declaration for one three-dimensional body.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PhysicsBody3D {
    pub id: PhysicsBodyId,
    pub kind: PhysicsBodyKind,
    pub pose: PhysicsPose3D,
    pub velocity: PhysicsVelocity3D,
    pub gravity_scale: f32,
    pub linear_damping: f32,
    pub angular_damping: f32,
    pub continuous_collision_detection: bool,
    pub colliders: Vec<Collider3D>,
}

impl PhysicsBody3D {
    pub fn dynamic(id: PhysicsBodyId, shape: PhysicsShape3D) -> Self {
        Self {
            id,
            kind: PhysicsBodyKind::Dynamic,
            pose: PhysicsPose3D::default(),
            velocity: PhysicsVelocity3D::default(),
            gravity_scale: 1.0,
            linear_damping: 0.0,
            angular_damping: 0.0,
            continuous_collision_detection: false,
            colliders: vec![Collider3D::new(shape)],
        }
    }

    pub fn fixed(id: PhysicsBodyId, shape: PhysicsShape3D) -> Self {
        Self {
            kind: PhysicsBodyKind::Fixed,
            ..Self::dynamic(id, shape)
        }
    }
}

/// Backend contract used by game code that opts into three-dimensional physics.
pub trait PhysicsProvider3D {
    type Error: std::error::Error + Send + Sync + 'static;

    fn insert_body(&mut self, body: PhysicsBody3D) -> Result<(), Self::Error>;
    fn remove_body(&mut self, id: &PhysicsBodyId) -> bool;
    fn contains_body(&self, id: &PhysicsBodyId) -> bool;
    fn body_pose(&self, id: &PhysicsBodyId) -> Option<PhysicsPose3D>;
    fn body_velocity(&self, id: &PhysicsBodyId) -> Option<PhysicsVelocity3D>;
    fn set_body_pose(
        &mut self,
        id: &PhysicsBodyId,
        pose: PhysicsPose3D,
        wake_up: bool,
    ) -> Result<(), Self::Error>;
    fn set_body_velocity(
        &mut self,
        id: &PhysicsBodyId,
        velocity: PhysicsVelocity3D,
        wake_up: bool,
    ) -> Result<(), Self::Error>;
    fn add_force(
        &mut self,
        id: &PhysicsBodyId,
        force: PhysicsVector3,
        wake_up: bool,
    ) -> Result<(), Self::Error>;
    fn add_force_at_point(
        &mut self,
        id: &PhysicsBodyId,
        force: PhysicsVector3,
        point: PhysicsVector3,
        wake_up: bool,
    ) -> Result<(), Self::Error>;
    fn apply_impulse(
        &mut self,
        id: &PhysicsBodyId,
        impulse: PhysicsVector3,
        wake_up: bool,
    ) -> Result<(), Self::Error>;
    fn cast_ray(
        &self,
        origin: PhysicsVector3,
        direction: PhysicsVector3,
        max_distance: f32,
        solid: bool,
    ) -> Result<Option<PhysicsRayHit3D>, Self::Error>;
    fn step(&mut self, duration: crate::StepDuration);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn physics_declarations_round_trip_without_provider_types() {
        let body = PhysicsBody2D::dynamic(
            PhysicsBodyId::from_key(&7_u64),
            PhysicsShape2D::Circle { radius: 0.5 },
        );
        let encoded = serde_json::to_string(&body).expect("serialize body");
        let decoded: PhysicsBody2D = serde_json::from_str(&encoded).expect("deserialize body");
        assert_eq!(decoded, body);
        assert!(!encoded.contains("rapier"));

        let body = PhysicsBody3D::dynamic(
            PhysicsBodyId::from_key(&8_u64),
            PhysicsShape3D::Cuboid {
                half_extents: PhysicsVector3::new(0.5, 1.0, 2.0),
            },
        );
        let encoded = serde_json::to_string(&body).expect("serialize 3D body");
        let decoded: PhysicsBody3D = serde_json::from_str(&encoded).expect("deserialize 3D body");
        assert_eq!(decoded, body);
        assert!(!encoded.contains("rapier"));
    }
}
