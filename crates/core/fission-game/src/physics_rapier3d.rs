//! Rapier implementation of Fission's backend-neutral 3D physics declarations.

use std::collections::BTreeMap;
use std::fmt;

use rapier3d::prelude::{
    ColliderBuilder, PhysicsWorld, Pose, RigidBodyBuilder, RigidBodyHandle, Rotation, Vector,
};

use crate::{
    Collider3D, PhysicsBody3D, PhysicsBodyId, PhysicsBodyKind, PhysicsPose3D, PhysicsProvider3D,
    PhysicsRotation3D, PhysicsShape3D, PhysicsVector3, PhysicsVelocity3D, StepDuration,
};

/// Invalid declaration or operation rejected by the Rapier 3D provider.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Physics3DError {
    message: String,
}

impl Physics3DError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for Physics3DError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for Physics3DError {}

/// Deterministic fixed-step 3D rigid-body world backed by Rapier.
pub struct RapierPhysicsWorld3D {
    world: PhysicsWorld,
    bodies: BTreeMap<PhysicsBodyId, RigidBodyHandle>,
}

impl RapierPhysicsWorld3D {
    pub fn new(gravity: PhysicsVector3) -> Result<Self, Physics3DError> {
        if !gravity.is_finite() {
            return Err(Physics3DError::new("physics gravity must be finite"));
        }
        let mut world = PhysicsWorld::new();
        world.gravity = Vector::new(gravity.x, gravity.y, gravity.z);
        Ok(Self {
            world,
            bodies: BTreeMap::new(),
        })
    }

    pub fn insert_body(&mut self, body: PhysicsBody3D) -> Result<(), Physics3DError> {
        validate_body(&body)?;
        if self.bodies.contains_key(&body.id) {
            return Err(Physics3DError::new(
                "physics body identity is already present",
            ));
        }

        let builder = match body.kind {
            PhysicsBodyKind::Dynamic => RigidBodyBuilder::dynamic(),
            PhysicsBodyKind::Fixed => RigidBodyBuilder::fixed(),
            PhysicsBodyKind::KinematicPosition => RigidBodyBuilder::kinematic_position_based(),
            PhysicsBodyKind::KinematicVelocity => RigidBodyBuilder::kinematic_velocity_based(),
        }
        .pose(isometry(body.pose))
        .linvel(vector(body.velocity.linear))
        .angvel(vector(body.velocity.angular))
        .gravity_scale(body.gravity_scale)
        .linear_damping(body.linear_damping)
        .angular_damping(body.angular_damping)
        .ccd_enabled(body.continuous_collision_detection);

        let handle = self.world.insert_body(builder);
        for collider in body.colliders {
            self.world
                .insert_collider(collider_builder(collider), Some(handle));
        }
        self.bodies.insert(body.id, handle);
        Ok(())
    }

    pub fn remove_body(&mut self, id: &PhysicsBodyId) -> bool {
        self.bodies
            .remove(id)
            .and_then(|handle| self.world.remove_body(handle))
            .is_some()
    }

    pub fn contains_body(&self, id: &PhysicsBodyId) -> bool {
        self.bodies.contains_key(id)
    }

    pub fn body_pose(&self, id: &PhysicsBodyId) -> Option<PhysicsPose3D> {
        let body = self.world.bodies.get(*self.bodies.get(id)?)?;
        let translation = body.translation();
        let rotation = body.rotation();
        Some(PhysicsPose3D {
            translation: PhysicsVector3::new(translation.x, translation.y, translation.z),
            rotation: PhysicsRotation3D::new(rotation.x, rotation.y, rotation.z, rotation.w),
        })
    }

    pub fn body_velocity(&self, id: &PhysicsBodyId) -> Option<PhysicsVelocity3D> {
        let body = self.world.bodies.get(*self.bodies.get(id)?)?;
        Some(PhysicsVelocity3D {
            linear: from_vector(body.linvel()),
            angular: from_vector(body.angvel()),
        })
    }

    pub fn step(&mut self, duration: StepDuration) {
        self.world.integration_parameters.dt = duration.as_secs_f32();
        self.world.step();
    }
}

impl PhysicsProvider3D for RapierPhysicsWorld3D {
    type Error = Physics3DError;

    fn insert_body(&mut self, body: PhysicsBody3D) -> Result<(), Self::Error> {
        RapierPhysicsWorld3D::insert_body(self, body)
    }

    fn remove_body(&mut self, id: &PhysicsBodyId) -> bool {
        RapierPhysicsWorld3D::remove_body(self, id)
    }

    fn contains_body(&self, id: &PhysicsBodyId) -> bool {
        RapierPhysicsWorld3D::contains_body(self, id)
    }

    fn body_pose(&self, id: &PhysicsBodyId) -> Option<PhysicsPose3D> {
        RapierPhysicsWorld3D::body_pose(self, id)
    }

    fn body_velocity(&self, id: &PhysicsBodyId) -> Option<PhysicsVelocity3D> {
        RapierPhysicsWorld3D::body_velocity(self, id)
    }

    fn step(&mut self, duration: StepDuration) {
        RapierPhysicsWorld3D::step(self, duration);
    }
}

fn validate_body(body: &PhysicsBody3D) -> Result<(), Physics3DError> {
    if !body.pose.is_valid()
        || !body.velocity.is_finite()
        || !body.gravity_scale.is_finite()
        || !body.linear_damping.is_finite()
        || !body.angular_damping.is_finite()
    {
        return Err(Physics3DError::new("physics body values must be valid"));
    }
    if body.linear_damping < 0.0 || body.angular_damping < 0.0 {
        return Err(Physics3DError::new(
            "physics body damping cannot be negative",
        ));
    }
    for collider in &body.colliders {
        validate_collider(collider)?;
    }
    Ok(())
}

fn validate_collider(collider: &Collider3D) -> Result<(), Physics3DError> {
    if !collider.offset.is_valid()
        || !collider.density.is_finite()
        || !collider.friction.is_finite()
        || !collider.restitution.is_finite()
    {
        return Err(Physics3DError::new("physics collider values must be valid"));
    }
    if collider.density < 0.0 || collider.friction < 0.0 || collider.restitution < 0.0 {
        return Err(Physics3DError::new(
            "physics collider material values cannot be negative",
        ));
    }
    let valid_shape = match collider.shape {
        PhysicsShape3D::Sphere { radius } => radius.is_finite() && radius > 0.0,
        PhysicsShape3D::Cuboid { half_extents } => {
            half_extents.is_finite()
                && half_extents.x > 0.0
                && half_extents.y > 0.0
                && half_extents.z > 0.0
        }
        PhysicsShape3D::CapsuleY {
            half_height,
            radius,
        } => half_height.is_finite() && radius.is_finite() && half_height >= 0.0 && radius > 0.0,
    };
    if !valid_shape {
        return Err(Physics3DError::new(
            "physics collider dimensions must be finite and positive",
        ));
    }
    Ok(())
}

fn collider_builder(collider: Collider3D) -> ColliderBuilder {
    let builder = match collider.shape {
        PhysicsShape3D::Sphere { radius } => ColliderBuilder::ball(radius),
        PhysicsShape3D::Cuboid { half_extents } => {
            ColliderBuilder::cuboid(half_extents.x, half_extents.y, half_extents.z)
        }
        PhysicsShape3D::CapsuleY {
            half_height,
            radius,
        } => ColliderBuilder::capsule_y(half_height, radius),
    };
    builder
        .position(isometry(collider.offset))
        .density(collider.density)
        .friction(collider.friction)
        .restitution(collider.restitution)
        .sensor(collider.sensor)
}

fn isometry(pose: PhysicsPose3D) -> Pose {
    let rotation = pose.rotation;
    Pose::from_parts(
        vector(pose.translation),
        Rotation::from_xyzw(rotation.x, rotation.y, rotation.z, rotation.w).normalize(),
    )
}

fn vector(value: PhysicsVector3) -> Vector {
    Vector::new(value.x, value.y, value.z)
}

fn from_vector(value: Vector) -> PhysicsVector3 {
    PhysicsVector3::new(value.x, value.y, value.z)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn body_id(value: u64) -> PhysicsBodyId {
        PhysicsBodyId::from_key(&value)
    }

    #[test]
    fn fixed_steps_advance_a_dynamic_body_without_exposing_rapier_handles() {
        let mut world = RapierPhysicsWorld3D::new(PhysicsVector3::new(0.0, -9.81, 0.0))
            .expect("finite gravity");
        let id = body_id(1);
        let mut body = PhysicsBody3D::dynamic(id.clone(), PhysicsShape3D::Sphere { radius: 0.5 });
        body.pose.translation.y = 5.0;
        world.insert_body(body).expect("insert body");

        world.step(StepDuration::from_hz(60));

        let pose = world.body_pose(&id).expect("body pose");
        let velocity = world.body_velocity(&id).expect("body velocity");
        assert!(pose.translation.y < 5.0);
        assert!(velocity.linear.y < 0.0);
    }

    #[test]
    fn zero_quaternions_are_rejected_before_rapier_receives_them() {
        let mut world = RapierPhysicsWorld3D::new(PhysicsVector3::ZERO).expect("finite gravity");
        let mut body = PhysicsBody3D::dynamic(body_id(2), PhysicsShape3D::Sphere { radius: 0.5 });
        body.pose.rotation = PhysicsRotation3D::new(0.0, 0.0, 0.0, 0.0);
        let error = world.insert_body(body).expect_err("invalid rotation");
        assert_eq!(error.to_string(), "physics body values must be valid");
    }
}
