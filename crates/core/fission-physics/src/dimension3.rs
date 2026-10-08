use serde::{Deserialize, Serialize};

use fission_game::StepDuration;
use fission_scene::{Quat, Vec3};

use crate::{
    ContactEvent, ContactPair, PhysicsBodyId, PhysicsBodyKind, PhysicsQueryFilter,
    PHYSICS_SNAPSHOT_VERSION,
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct PhysicsPose3D {
    pub translation: Vec3,
    pub rotation: Quat,
}

impl PhysicsPose3D {
    pub const fn new(translation: Vec3, rotation: Quat) -> Self {
        Self {
            translation,
            rotation,
        }
    }

    pub fn is_valid(self) -> bool {
        self.translation.is_finite() && self.rotation.is_valid()
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct PhysicsVelocity3D {
    pub linear: Vec3,
    pub angular_axis_radians_per_second: Vec3,
}

impl PhysicsVelocity3D {
    pub const fn new(linear: Vec3, angular_axis_radians_per_second: Vec3) -> Self {
        Self {
            linear,
            angular_axis_radians_per_second,
        }
    }

    pub fn is_valid(self) -> bool {
        self.linear.is_finite() && self.angular_axis_radians_per_second.is_finite()
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "shape")]
pub enum PhysicsShape3D {
    Sphere { radius: f32 },
    Cuboid { half_extents: Vec3 },
    CapsuleY { half_height: f32, radius: f32 },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Collider3D {
    pub shape: PhysicsShape3D,
    pub offset: PhysicsPose3D,
    pub density: f32,
    pub friction: f32,
    pub restitution: f32,
    pub trigger: bool,
}

impl Collider3D {
    pub fn new(shape: PhysicsShape3D) -> Self {
        Self {
            shape,
            offset: PhysicsPose3D::default(),
            density: 1.0,
            friction: 0.5,
            restitution: 0.0,
            trigger: false,
        }
    }
}

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
        Self::new(id, PhysicsBodyKind::Dynamic, shape)
    }

    pub fn fixed(id: PhysicsBodyId, shape: PhysicsShape3D) -> Self {
        Self::new(id, PhysicsBodyKind::Fixed, shape)
    }

    pub fn kinematic(id: PhysicsBodyId, shape: PhysicsShape3D) -> Self {
        Self::new(id, PhysicsBodyKind::Kinematic, shape)
    }

    fn new(id: PhysicsBodyId, kind: PhysicsBodyKind, shape: PhysicsShape3D) -> Self {
        Self {
            id,
            kind,
            pose: PhysicsPose3D::default(),
            velocity: PhysicsVelocity3D::default(),
            gravity_scale: 1.0,
            linear_damping: 0.0,
            angular_damping: 0.0,
            continuous_collision_detection: false,
            colliders: vec![Collider3D::new(shape)],
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PhysicsRayHit3D {
    pub body: PhysicsBodyId,
    pub distance: f32,
    pub point: Vec3,
    pub normal: Vec3,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct CharacterController3D {
    pub up: Vec3,
    pub offset: f32,
    pub slide: bool,
    pub max_slope_climb_radians: f32,
    pub min_slope_slide_radians: f32,
    pub snap_to_ground: Option<f32>,
}

impl Default for CharacterController3D {
    fn default() -> Self {
        Self {
            up: Vec3::new(0.0, 1.0, 0.0),
            offset: 0.01,
            slide: true,
            max_slope_climb_radians: 45.0_f32.to_radians(),
            min_slope_slide_radians: 30.0_f32.to_radians(),
            snap_to_ground: Some(0.2),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CharacterMovement3D {
    pub translation: Vec3,
    pub grounded: bool,
    pub sliding_down_slope: bool,
    pub collisions: Vec<PhysicsBodyId>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PhysicsBodyState3D {
    pub body: PhysicsBody3D,
    pub sleeping: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PhysicsSnapshot3D {
    pub version: u32,
    pub gravity: Vec3,
    pub bodies: Vec<PhysicsBodyState3D>,
    pub contacts: Vec<ContactPair>,
}

impl PhysicsSnapshot3D {
    pub fn new(gravity: Vec3) -> Self {
        Self {
            version: PHYSICS_SNAPSHOT_VERSION,
            gravity,
            bodies: Vec::new(),
            contacts: Vec::new(),
        }
    }
}

pub trait PhysicsProvider3D {
    type Error: std::error::Error + Send + Sync + 'static;

    fn insert_body(&mut self, body: PhysicsBody3D) -> Result<(), Self::Error>;
    fn remove_body(&mut self, id: PhysicsBodyId) -> bool;
    fn contains_body(&self, id: PhysicsBodyId) -> bool;
    fn body_pose(&self, id: PhysicsBodyId) -> Option<PhysicsPose3D>;
    fn body_velocity(&self, id: PhysicsBodyId) -> Option<PhysicsVelocity3D>;
    fn set_body_pose(
        &mut self,
        id: PhysicsBodyId,
        pose: PhysicsPose3D,
        wake: bool,
    ) -> Result<(), Self::Error>;
    fn set_body_velocity(
        &mut self,
        id: PhysicsBodyId,
        velocity: PhysicsVelocity3D,
        wake: bool,
    ) -> Result<(), Self::Error>;
    fn add_force(&mut self, id: PhysicsBodyId, force: Vec3, wake: bool) -> Result<(), Self::Error>;
    fn apply_impulse(
        &mut self,
        id: PhysicsBodyId,
        impulse: Vec3,
        wake: bool,
    ) -> Result<(), Self::Error>;
    fn cast_ray(
        &self,
        origin: Vec3,
        direction: Vec3,
        max_distance: f32,
        filter: PhysicsQueryFilter,
    ) -> Result<Option<PhysicsRayHit3D>, Self::Error>;
    fn overlap_shape(
        &self,
        shape: &PhysicsShape3D,
        pose: PhysicsPose3D,
        filter: PhysicsQueryFilter,
    ) -> Result<Vec<PhysicsBodyId>, Self::Error>;
    fn move_character(
        &mut self,
        body: PhysicsBodyId,
        desired_translation: Vec3,
        duration: StepDuration,
        controller: CharacterController3D,
    ) -> Result<CharacterMovement3D, Self::Error>;
    fn contacts(&self) -> &[ContactPair];
    fn drain_contact_events(&mut self) -> Vec<ContactEvent>;
    fn step(&mut self, duration: StepDuration);
    fn snapshot(&self) -> PhysicsSnapshot3D;
    fn restore(&mut self, snapshot: PhysicsSnapshot3D) -> Result<(), Self::Error>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn declarations_round_trip_without_backend_types() {
        let body = PhysicsBody3D::dynamic(
            PhysicsBodyId::new(8),
            PhysicsShape3D::Cuboid {
                half_extents: Vec3::new(0.5, 1.0, 2.0),
            },
        );
        let encoded = serde_json::to_string(&body).expect("serialize body");
        assert_eq!(
            serde_json::from_str::<PhysicsBody3D>(&encoded).unwrap(),
            body
        );
        assert!(!encoded.contains("rapier"));
    }
}
