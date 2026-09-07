//! Rapier implementation of Fission's backend-neutral 2D physics declarations.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use rapier2d::prelude::{
    ColliderBuilder, PhysicsWorld, Pose, QueryFilter, Ray, RigidBody, RigidBodyBuilder,
    RigidBodyHandle, Rotation, Vector,
};

use crate::{
    Collider2D, PhysicsBody2D, PhysicsBodyId, PhysicsBodyKind, PhysicsContact, PhysicsContactEvent,
    PhysicsContactEventKind, PhysicsPose2D, PhysicsProvider2D, PhysicsRayHit2D, PhysicsShape2D,
    PhysicsVector2, PhysicsVelocity2D, StepDuration,
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
    contacts: Vec<PhysicsContact>,
    contact_events: Vec<PhysicsContactEvent>,
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
            contacts: Vec::new(),
            contact_events: Vec::new(),
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

    pub fn set_body_pose(
        &mut self,
        id: &PhysicsBodyId,
        pose: PhysicsPose2D,
        wake_up: bool,
    ) -> Result<(), Physics2DError> {
        if !pose.is_finite() {
            return Err(Physics2DError::new("physics pose must be finite"));
        }
        self.body_mut(id)?.set_position(isometry(pose), wake_up);
        Ok(())
    }

    pub fn set_body_velocity(
        &mut self,
        id: &PhysicsBodyId,
        velocity: PhysicsVelocity2D,
        wake_up: bool,
    ) -> Result<(), Physics2DError> {
        if !velocity.is_finite() {
            return Err(Physics2DError::new("physics velocity must be finite"));
        }
        let body = self.body_mut(id)?;
        body.set_linvel(vector(velocity.linear), wake_up);
        body.set_angvel(velocity.angular, wake_up);
        Ok(())
    }

    pub fn add_force(
        &mut self,
        id: &PhysicsBodyId,
        force: PhysicsVector2,
        wake_up: bool,
    ) -> Result<(), Physics2DError> {
        if !force.is_finite() {
            return Err(Physics2DError::new("physics force must be finite"));
        }
        self.body_mut(id)?.add_force(vector(force), wake_up);
        Ok(())
    }

    pub fn add_force_at_point(
        &mut self,
        id: &PhysicsBodyId,
        force: PhysicsVector2,
        point: PhysicsVector2,
        wake_up: bool,
    ) -> Result<(), Physics2DError> {
        if !force.is_finite() || !point.is_finite() {
            return Err(Physics2DError::new(
                "physics force and application point must be finite",
            ));
        }
        self.body_mut(id)?
            .add_force_at_point(vector(force), vector(point), wake_up);
        Ok(())
    }

    pub fn apply_impulse(
        &mut self,
        id: &PhysicsBodyId,
        impulse: PhysicsVector2,
        wake_up: bool,
    ) -> Result<(), Physics2DError> {
        if !impulse.is_finite() {
            return Err(Physics2DError::new("physics impulse must be finite"));
        }
        self.body_mut(id)?.apply_impulse(vector(impulse), wake_up);
        Ok(())
    }

    pub fn cast_ray(
        &self,
        origin: PhysicsVector2,
        direction: PhysicsVector2,
        max_distance: f32,
        solid: bool,
    ) -> Result<Option<PhysicsRayHit2D>, Physics2DError> {
        let (origin, direction) = validated_ray2(origin, direction, max_distance)?;
        let ray = Ray::new(origin, direction);
        let Some((collider_handle, intersection)) =
            self.world
                .cast_ray_and_get_normal(&ray, max_distance, solid, QueryFilter::default())
        else {
            return Ok(None);
        };
        let body_handle = self
            .world
            .colliders
            .get(collider_handle)
            .and_then(|collider| collider.parent())
            .ok_or_else(|| Physics2DError::new("physics ray hit an unattached collider"))?;
        let body = self
            .bodies
            .iter()
            .find_map(|(id, handle)| (*handle == body_handle).then(|| id.clone()))
            .ok_or_else(|| Physics2DError::new("physics ray hit an unknown body"))?;
        let point = ray.point_at(intersection.time_of_impact);
        Ok(Some(PhysicsRayHit2D {
            body,
            distance: intersection.time_of_impact,
            point: from_vector(point),
            normal: from_vector(intersection.normal),
        }))
    }

    pub fn contacts(&self) -> &[PhysicsContact] {
        &self.contacts
    }

    pub fn drain_contact_events(&mut self) -> Vec<PhysicsContactEvent> {
        std::mem::take(&mut self.contact_events)
    }

    pub fn step(&mut self, duration: StepDuration) {
        self.world.integration_parameters.dt = duration.as_secs_f32();
        self.world.step();
        self.update_contacts();
    }

    fn body_mut(&mut self, id: &PhysicsBodyId) -> Result<&mut RigidBody, Physics2DError> {
        let handle = *self
            .bodies
            .get(id)
            .ok_or_else(|| Physics2DError::new("physics body identity is not present"))?;
        self.world
            .bodies
            .get_mut(handle)
            .ok_or_else(|| Physics2DError::new("physics body handle is stale"))
    }

    fn update_contacts(&mut self) {
        let previous = self.contacts.iter().cloned().collect::<BTreeSet<_>>();
        let mut current = BTreeSet::new();
        for pair in self
            .world
            .contact_pairs()
            .filter(|pair| pair.has_any_active_contact())
        {
            if let Some(contact) = self.contact_for_colliders(pair.collider1, pair.collider2, false)
            {
                current.insert(contact);
            }
        }
        for (first, _, second, _, intersecting) in self.world.intersection_pairs() {
            if intersecting {
                if let Some(contact) = self.contact_for_colliders(first, second, true) {
                    current.insert(contact);
                }
            }
        }
        self.contact_events
            .extend(
                current
                    .difference(&previous)
                    .cloned()
                    .map(|contact| PhysicsContactEvent {
                        kind: PhysicsContactEventKind::Started,
                        contact,
                    }),
            );
        self.contact_events
            .extend(
                previous
                    .difference(&current)
                    .cloned()
                    .map(|contact| PhysicsContactEvent {
                        kind: PhysicsContactEventKind::Stopped,
                        contact,
                    }),
            );
        self.contacts = current.into_iter().collect();
    }

    fn contact_for_colliders(
        &self,
        first: rapier2d::prelude::ColliderHandle,
        second: rapier2d::prelude::ColliderHandle,
        sensor: bool,
    ) -> Option<PhysicsContact> {
        let first = self.body_id_for_collider(first)?;
        let second = self.body_id_for_collider(second)?;
        (first != second).then(|| PhysicsContact::new(first, second, sensor))
    }

    fn body_id_for_collider(
        &self,
        collider: rapier2d::prelude::ColliderHandle,
    ) -> Option<PhysicsBodyId> {
        let body = self.world.colliders.get(collider)?.parent()?;
        self.bodies
            .iter()
            .find_map(|(id, handle)| (*handle == body).then(|| id.clone()))
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

    fn set_body_pose(
        &mut self,
        id: &PhysicsBodyId,
        pose: PhysicsPose2D,
        wake_up: bool,
    ) -> Result<(), Self::Error> {
        RapierPhysicsWorld2D::set_body_pose(self, id, pose, wake_up)
    }

    fn set_body_velocity(
        &mut self,
        id: &PhysicsBodyId,
        velocity: PhysicsVelocity2D,
        wake_up: bool,
    ) -> Result<(), Self::Error> {
        RapierPhysicsWorld2D::set_body_velocity(self, id, velocity, wake_up)
    }

    fn add_force(
        &mut self,
        id: &PhysicsBodyId,
        force: PhysicsVector2,
        wake_up: bool,
    ) -> Result<(), Self::Error> {
        RapierPhysicsWorld2D::add_force(self, id, force, wake_up)
    }

    fn add_force_at_point(
        &mut self,
        id: &PhysicsBodyId,
        force: PhysicsVector2,
        point: PhysicsVector2,
        wake_up: bool,
    ) -> Result<(), Self::Error> {
        RapierPhysicsWorld2D::add_force_at_point(self, id, force, point, wake_up)
    }

    fn apply_impulse(
        &mut self,
        id: &PhysicsBodyId,
        impulse: PhysicsVector2,
        wake_up: bool,
    ) -> Result<(), Self::Error> {
        RapierPhysicsWorld2D::apply_impulse(self, id, impulse, wake_up)
    }

    fn cast_ray(
        &self,
        origin: PhysicsVector2,
        direction: PhysicsVector2,
        max_distance: f32,
        solid: bool,
    ) -> Result<Option<PhysicsRayHit2D>, Self::Error> {
        RapierPhysicsWorld2D::cast_ray(self, origin, direction, max_distance, solid)
    }

    fn contacts(&self) -> &[PhysicsContact] {
        RapierPhysicsWorld2D::contacts(self)
    }

    fn drain_contact_events(&mut self) -> Vec<PhysicsContactEvent> {
        RapierPhysicsWorld2D::drain_contact_events(self)
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

fn vector(value: PhysicsVector2) -> Vector {
    Vector::new(value.x, value.y)
}

fn from_vector(value: Vector) -> PhysicsVector2 {
    PhysicsVector2::new(value.x, value.y)
}

fn validated_ray2(
    origin: PhysicsVector2,
    direction: PhysicsVector2,
    max_distance: f32,
) -> Result<(Vector, Vector), Physics2DError> {
    let origin = vector(origin);
    let direction = vector(direction);
    if !origin.is_finite()
        || !direction.is_finite()
        || direction.length_squared() <= f32::EPSILON
        || !max_distance.is_finite()
        || max_distance < 0.0
    {
        return Err(Physics2DError::new(
            "physics ray must have finite values, a direction, and a nonnegative distance",
        ));
    }
    Ok((origin, direction.normalize()))
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

    #[test]
    fn forces_and_authoritative_updates_use_stable_body_ids() {
        let mut world = RapierPhysicsWorld2D::new(PhysicsVector2::ZERO).expect("finite gravity");
        let id = body_id(3);
        world
            .insert_body(PhysicsBody2D::dynamic(
                id.clone(),
                PhysicsShape2D::Circle { radius: 0.5 },
            ))
            .expect("insert body");
        world
            .set_body_pose(
                &id,
                PhysicsPose2D::new(PhysicsVector2::new(2.0, 3.0), 0.25),
                true,
            )
            .expect("set pose");
        world
            .set_body_velocity(
                &id,
                PhysicsVelocity2D::new(PhysicsVector2::new(1.0, 0.0), 0.0),
                true,
            )
            .expect("set velocity");
        world
            .add_force_at_point(
                &id,
                PhysicsVector2::new(10.0, 0.0),
                PhysicsVector2::new(2.0, 4.0),
                true,
            )
            .expect("apply off-center force");
        world
            .apply_impulse(&id, PhysicsVector2::new(1.0, 0.0), true)
            .expect("apply impulse");

        world.step(StepDuration::from_hz(60));

        let pose = world.body_pose(&id).expect("body pose");
        let velocity = world.body_velocity(&id).expect("body velocity");
        assert!(pose.translation.x > 2.0);
        assert!(velocity.linear.x > 1.0);
        assert!(velocity.angular.abs() > 0.0);
    }

    #[test]
    fn ray_queries_return_stable_body_identity_and_surface_data() {
        let mut world = RapierPhysicsWorld2D::new(PhysicsVector2::ZERO).expect("finite gravity");
        let id = body_id(4);
        world
            .insert_body(PhysicsBody2D::fixed(
                id.clone(),
                PhysicsShape2D::Circle { radius: 0.5 },
            ))
            .expect("insert body");
        world.step(StepDuration::from_hz(60));

        let hit = world
            .cast_ray(
                PhysicsVector2::new(0.0, 5.0),
                PhysicsVector2::new(0.0, -2.0),
                10.0,
                true,
            )
            .expect("valid ray")
            .expect("ray should hit body");

        assert_eq!(hit.body, id);
        assert!((hit.distance - 4.5).abs() < 1.0e-4);
        assert!((hit.point.y - 0.5).abs() < 1.0e-4);
        assert!((hit.normal.y - 1.0).abs() < 1.0e-4);
    }

    #[test]
    fn contact_transitions_are_deterministic_and_use_stable_ids() {
        let mut world = RapierPhysicsWorld2D::new(PhysicsVector2::ZERO).expect("finite gravity");
        let first = body_id(5);
        let second = body_id(6);
        world
            .insert_body(PhysicsBody2D::fixed(
                first.clone(),
                PhysicsShape2D::Circle { radius: 1.0 },
            ))
            .expect("insert first body");
        let mut overlapping =
            PhysicsBody2D::dynamic(second.clone(), PhysicsShape2D::Circle { radius: 1.0 });
        overlapping.pose.translation.x = 1.5;
        world.insert_body(overlapping).expect("insert second body");

        world.step(StepDuration::from_hz(60));
        let contact = PhysicsContact::new(first, second.clone(), false);
        assert_eq!(world.contacts(), std::slice::from_ref(&contact));
        assert_eq!(
            world.drain_contact_events(),
            vec![PhysicsContactEvent {
                kind: PhysicsContactEventKind::Started,
                contact: contact.clone(),
            }]
        );
        assert!(world.drain_contact_events().is_empty());

        world
            .set_body_pose(
                &second,
                PhysicsPose2D::new(PhysicsVector2::new(10.0, 0.0), 0.0),
                true,
            )
            .expect("move second body away");
        world.step(StepDuration::from_hz(60));
        assert!(world.contacts().is_empty());
        assert_eq!(
            world.drain_contact_events(),
            vec![PhysicsContactEvent {
                kind: PhysicsContactEventKind::Stopped,
                contact,
            }]
        );
    }

    #[test]
    fn sensor_intersections_are_distinguished_from_solid_contacts() {
        let mut world = RapierPhysicsWorld2D::new(PhysicsVector2::ZERO).expect("finite gravity");
        let zone = body_id(7);
        let actor = body_id(8);
        let mut sensor = PhysicsBody2D::fixed(zone.clone(), PhysicsShape2D::Circle { radius: 2.0 });
        sensor.colliders[0].sensor = true;
        world.insert_body(sensor).expect("insert sensor");
        world
            .insert_body(PhysicsBody2D::dynamic(
                actor.clone(),
                PhysicsShape2D::Circle { radius: 0.5 },
            ))
            .expect("insert actor");

        world.step(StepDuration::from_hz(60));

        assert_eq!(world.contacts(), &[PhysicsContact::new(zone, actor, true)]);
    }
}
