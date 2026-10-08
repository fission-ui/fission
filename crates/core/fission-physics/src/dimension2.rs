use serde::{Deserialize, Serialize};

use fission_game::StepDuration;
use fission_scene::Vec2;

use crate::{
    ContactEvent, ContactPair, PhysicsBodyId, PhysicsBodyKind, PhysicsQueryFilter,
    PHYSICS_SNAPSHOT_VERSION,
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct PhysicsPose2D {
    pub translation: Vec2,
    pub rotation_radians: f32,
}

impl PhysicsPose2D {
    pub const fn new(translation: Vec2, rotation_radians: f32) -> Self {
        Self {
            translation,
            rotation_radians,
        }
    }

    pub fn is_valid(self) -> bool {
        self.translation.is_finite() && self.rotation_radians.is_finite()
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct PhysicsVelocity2D {
    pub linear: Vec2,
    pub angular_radians_per_second: f32,
}

impl PhysicsVelocity2D {
    pub const fn new(linear: Vec2, angular_radians_per_second: f32) -> Self {
        Self {
            linear,
            angular_radians_per_second,
        }
    }

    pub fn is_valid(self) -> bool {
        self.linear.is_finite() && self.angular_radians_per_second.is_finite()
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "shape")]
pub enum PhysicsShape2D {
    Circle { radius: f32 },
    Cuboid { half_extents: Vec2 },
    CapsuleY { half_height: f32, radius: f32 },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Collider2D {
    pub shape: PhysicsShape2D,
    pub offset: PhysicsPose2D,
    pub density: f32,
    pub friction: f32,
    pub restitution: f32,
    pub trigger: bool,
}

impl Collider2D {
    pub fn new(shape: PhysicsShape2D) -> Self {
        Self {
            shape,
            offset: PhysicsPose2D::default(),
            density: 1.0,
            friction: 0.5,
            restitution: 0.0,
            trigger: false,
        }
    }
}

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
        Self::new(id, PhysicsBodyKind::Dynamic, shape)
    }

    pub fn fixed(id: PhysicsBodyId, shape: PhysicsShape2D) -> Self {
        Self::new(id, PhysicsBodyKind::Fixed, shape)
    }

    pub fn kinematic(id: PhysicsBodyId, shape: PhysicsShape2D) -> Self {
        Self::new(id, PhysicsBodyKind::Kinematic, shape)
    }

    fn new(id: PhysicsBodyId, kind: PhysicsBodyKind, shape: PhysicsShape2D) -> Self {
        Self {
            id,
            kind,
            pose: PhysicsPose2D::default(),
            velocity: PhysicsVelocity2D::default(),
            gravity_scale: 1.0,
            linear_damping: 0.0,
            angular_damping: 0.0,
            continuous_collision_detection: false,
            colliders: vec![Collider2D::new(shape)],
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PhysicsRayHit2D {
    pub body: PhysicsBodyId,
    pub distance: f32,
    pub point: Vec2,
    pub normal: Vec2,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PhysicsBodyState2D {
    pub body: PhysicsBody2D,
    pub sleeping: bool,
}

/// Portable snapshot captured between fixed steps.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PhysicsSnapshot2D {
    pub version: u32,
    pub gravity: Vec2,
    pub bodies: Vec<PhysicsBodyState2D>,
    pub contacts: Vec<ContactPair>,
}

impl PhysicsSnapshot2D {
    pub fn new(gravity: Vec2) -> Self {
        Self {
            version: PHYSICS_SNAPSHOT_VERSION,
            gravity,
            bodies: Vec::new(),
            contacts: Vec::new(),
        }
    }
}

pub trait PhysicsProvider2D {
    type Error: std::error::Error + Send + Sync + 'static;

    fn insert_body(&mut self, body: PhysicsBody2D) -> Result<(), Self::Error>;
    fn remove_body(&mut self, id: PhysicsBodyId) -> bool;
    fn contains_body(&self, id: PhysicsBodyId) -> bool;
    fn body_pose(&self, id: PhysicsBodyId) -> Option<PhysicsPose2D>;
    fn body_velocity(&self, id: PhysicsBodyId) -> Option<PhysicsVelocity2D>;
    fn set_body_pose(
        &mut self,
        id: PhysicsBodyId,
        pose: PhysicsPose2D,
        wake: bool,
    ) -> Result<(), Self::Error>;
    fn set_body_velocity(
        &mut self,
        id: PhysicsBodyId,
        velocity: PhysicsVelocity2D,
        wake: bool,
    ) -> Result<(), Self::Error>;
    fn add_force(&mut self, id: PhysicsBodyId, force: Vec2, wake: bool) -> Result<(), Self::Error>;
    fn apply_impulse(
        &mut self,
        id: PhysicsBodyId,
        impulse: Vec2,
        wake: bool,
    ) -> Result<(), Self::Error>;
    fn cast_ray(
        &self,
        origin: Vec2,
        direction: Vec2,
        max_distance: f32,
        filter: PhysicsQueryFilter,
    ) -> Result<Option<PhysicsRayHit2D>, Self::Error>;
    fn overlap_shape(
        &self,
        shape: &PhysicsShape2D,
        pose: PhysicsPose2D,
        filter: PhysicsQueryFilter,
    ) -> Result<Vec<PhysicsBodyId>, Self::Error>;
    fn contacts(&self) -> &[ContactPair];
    fn drain_contact_events(&mut self) -> Vec<ContactEvent>;
    fn step(&mut self, duration: StepDuration);
    fn snapshot(&self) -> PhysicsSnapshot2D;
    fn restore(&mut self, snapshot: PhysicsSnapshot2D) -> Result<(), Self::Error>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn declarations_round_trip_without_backend_types() {
        let body = PhysicsBody2D::dynamic(
            PhysicsBodyId::new(7),
            PhysicsShape2D::Circle { radius: 0.5 },
        );
        let encoded = serde_json::to_string(&body).expect("serialize body");
        assert_eq!(
            serde_json::from_str::<PhysicsBody2D>(&encoded).unwrap(),
            body
        );
        assert!(!encoded.contains("rapier"));
    }
}
