//! Rapier implementation of Fission's backend-neutral 2D physics declarations.

use std::collections::BTreeMap;
use std::fmt;

use rapier2d::prelude::{
    ColliderBuilder, PhysicsWorld, Pose, RigidBodyBuilder, RigidBodyHandle, Rotation, Vector,
};

use crate::{
    Collider2D, PhysicsBody2D, PhysicsBodyId, PhysicsBodyKind, PhysicsPose2D, PhysicsProvider2D,
    PhysicsShape2D, PhysicsVector2, PhysicsVelocity2D, StepDuration,
};

/// Invalid declaration or operation rejected by the Rapier provider.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Physics2DError {
    message: String,
}

impl Physics2DError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for Physics2DError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for Physics2DError {}

/// Deterministic fixed-step 2D rigid-body world backed by Rapier.
///
/// The wrapper retains only stable Fission body IDs at its public boundary.
pub struct RapierPhysicsWorld2D {
    world: PhysicsWorld,
    bodies: BTreeMap<PhysicsBodyId, RigidBodyHandle>,
}

impl RapierPhysicsWorld2D {
    pub fn new(gravity: PhysicsVector2) -> Result<Self, Physics2DError> {
        if !gravity.is_finite() {
            return Err(Physics2DError::new("physics gravity must be finite"));
        }
        let mut world = PhysicsWorld::new();
        world.gravity = Vector::new(gravity.x, gravity.y);
        Ok(Self {
            world,
            bodies: BTreeMap::new(),
        })
    }

    pub fn insert_body(&mut self, body: PhysicsBody2D) -> Result<(), Physics2DError> {
        validate_body(&body)?;
        if self.bodies.contains_key(&body.id) {
            return Err(Physics2DError::new(
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
        .linvel(Vector::new(body.velocity.linear.x, body.velocity.linear.y))
        .angvel(body.velocity.angular)
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

    pub fn body_pose(&self, id: &PhysicsBodyId) -> Option<PhysicsPose2D> {
        let body = self.world.bodies.get(*self.bodies.get(id)?)?;
        let translation = body.translation();
        Some(PhysicsPose2D {
            translation: PhysicsVector2::new(translation.x, translation.y),
            rotation: body.rotation().angle(),
        })
    }

    pub fn body_velocity(&self, id: &PhysicsBodyId) -> Option<PhysicsVelocity2D> {
        let body = self.world.bodies.get(*self.bodies.get(id)?)?;
        Some(PhysicsVelocity2D {
            linear: PhysicsVector2::new(body.linvel().x, body.linvel().y),
            angular: body.angvel(),
        })
    }

    pub fn step(&mut self, duration: StepDuration) {
        self.world.integration_parameters.dt = duration.as_secs_f32();
        self.world.step();
    }
}

impl PhysicsProvider2D for RapierPhysicsWorld2D {
    type Error = Physics2DError;

    fn insert_body(&mut self, body: PhysicsBody2D) -> Result<(), Self::Error> {
        RapierPhysicsWorld2D::insert_body(self, body)
    }

    fn remove_body(&mut self, id: &PhysicsBodyId) -> bool {
        RapierPhysicsWorld2D::remove_body(self, id)
    }

    fn contains_body(&self, id: &PhysicsBodyId) -> bool {
        RapierPhysicsWorld2D::contains_body(self, id)
    }

    fn body_pose(&self, id: &PhysicsBodyId) -> Option<PhysicsPose2D> {
        RapierPhysicsWorld2D::body_pose(self, id)
    }

    fn body_velocity(&self, id: &PhysicsBodyId) -> Option<PhysicsVelocity2D> {
        RapierPhysicsWorld2D::body_velocity(self, id)
    }

    fn step(&mut self, duration: StepDuration) {
        RapierPhysicsWorld2D::step(self, duration);
    }
}

fn validate_body(body: &PhysicsBody2D) -> Result<(), Physics2DError> {
    if !body.pose.is_finite()
        || !body.velocity.is_finite()
        || !body.gravity_scale.is_finite()
        || !body.linear_damping.is_finite()
        || !body.angular_damping.is_finite()
    {
        return Err(Physics2DError::new("physics body values must be finite"));
    }
    if body.linear_damping < 0.0 || body.angular_damping < 0.0 {
        return Err(Physics2DError::new(
            "physics body damping cannot be negative",
        ));
    }
    for collider in &body.colliders {
        validate_collider(collider)?;
    }
    Ok(())
}

fn validate_collider(collider: &Collider2D) -> Result<(), Physics2DError> {
    if !collider.offset.is_finite()
        || !collider.density.is_finite()
        || !collider.friction.is_finite()
        || !collider.restitution.is_finite()
    {
        return Err(Physics2DError::new(
            "physics collider values must be finite",
        ));
    }
    if collider.density < 0.0 || collider.friction < 0.0 || collider.restitution < 0.0 {
        return Err(Physics2DError::new(
            "physics collider material values cannot be negative",
        ));
    }
    let valid_shape = match collider.shape {
        PhysicsShape2D::Circle { radius } => radius.is_finite() && radius > 0.0,
        PhysicsShape2D::Cuboid { half_extents } => {
            half_extents.is_finite() && half_extents.x > 0.0 && half_extents.y > 0.0
        }
        PhysicsShape2D::CapsuleY {
            half_height,
            radius,
        } => half_height.is_finite() && radius.is_finite() && half_height >= 0.0 && radius > 0.0,
    };
    if !valid_shape {
        return Err(Physics2DError::new(
            "physics collider dimensions must be finite and positive",
        ));
    }
    Ok(())
}

fn collider_builder(collider: Collider2D) -> ColliderBuilder {
    let builder = match collider.shape {
        PhysicsShape2D::Circle { radius } => ColliderBuilder::ball(radius),
        PhysicsShape2D::Cuboid { half_extents } => {
            ColliderBuilder::cuboid(half_extents.x, half_extents.y)
        }
        PhysicsShape2D::CapsuleY {
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

fn isometry(pose: PhysicsPose2D) -> Pose {
    Pose::from_parts(
        Vector::new(pose.translation.x, pose.translation.y),
        Rotation::new(pose.rotation),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn body_id(value: u64) -> PhysicsBodyId {
        PhysicsBodyId::from_key(&value)
    }

    #[test]
    fn fixed_steps_advance_a_dynamic_body_without_exposing_rapier_handles() {
        let mut world =
            RapierPhysicsWorld2D::new(PhysicsVector2::new(0.0, -9.81)).expect("finite gravity");
        let id = body_id(1);
        let mut body = PhysicsBody2D::dynamic(id.clone(), PhysicsShape2D::Circle { radius: 0.5 });
        body.pose.translation.y = 5.0;
        world.insert_body(body).expect("insert body");

        world.step(StepDuration::from_hz(60));

        let pose = world.body_pose(&id).expect("body pose");
        let velocity = world.body_velocity(&id).expect("body velocity");
        assert!(pose.translation.y < 5.0);
        assert!(velocity.linear.y < 0.0);
    }

    #[test]
    fn declarations_are_validated_before_rapier_receives_them() {
        let mut world = RapierPhysicsWorld2D::new(PhysicsVector2::ZERO).expect("finite gravity");
        let body = PhysicsBody2D::dynamic(body_id(2), PhysicsShape2D::Circle { radius: f32::NAN });
        let error = world.insert_body(body).expect_err("invalid circle");
        assert_eq!(
            error.to_string(),
            "physics collider dimensions must be finite and positive"
        );
    }
}
