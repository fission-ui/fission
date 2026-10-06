//! Rapier implementation of Fission's backend-neutral 2D physics contract.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use fission_physics::{
    Collider2D, ContactEvent, ContactKind, ContactPair, ContactTransition, PhysicsBody2D,
    PhysicsBodyId, PhysicsBodyKind, PhysicsBodyState2D, PhysicsPose2D, PhysicsProvider2D,
    PhysicsQueryFilter, PhysicsRayHit2D, PhysicsShape2D, PhysicsSnapshot2D, PhysicsVelocity2D,
    StepDuration, Vec2, PHYSICS_SNAPSHOT_VERSION,
};
use rapier2d::prelude::{
    ColliderBuilder, ColliderHandle, PhysicsWorld, Pose, QueryFilter, QueryFilterFlags, Ray,
    RigidBody, RigidBodyBuilder, RigidBodyHandle, Rotation, SharedShape, Vector,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RapierPhysics2DError {
    message: String,
}

impl RapierPhysics2DError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for RapierPhysics2DError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for RapierPhysics2DError {}

/// Fixed-step 2D world whose public boundary contains no Rapier handles.
pub struct RapierPhysicsWorld2D {
    gravity: Vec2,
    world: PhysicsWorld,
    handles: BTreeMap<PhysicsBodyId, RigidBodyHandle>,
    declarations: BTreeMap<PhysicsBodyId, PhysicsBody2D>,
    contacts: Vec<ContactPair>,
    contact_events: Vec<ContactEvent>,
}

impl RapierPhysicsWorld2D {
    pub fn new(gravity: Vec2) -> Result<Self, RapierPhysics2DError> {
        if !gravity.is_finite() {
            return Err(RapierPhysics2DError::new("physics gravity must be finite"));
        }
        let mut world = PhysicsWorld::new();
        world.gravity = vector(gravity);
        Ok(Self {
            gravity,
            world,
            handles: BTreeMap::new(),
            declarations: BTreeMap::new(),
            contacts: Vec::new(),
            contact_events: Vec::new(),
        })
    }

    fn body_mut(&mut self, id: PhysicsBodyId) -> Result<&mut RigidBody, RapierPhysics2DError> {
        let handle = *self
            .handles
            .get(&id)
            .ok_or_else(|| RapierPhysics2DError::new("physics body identity is not present"))?;
        self.world
            .bodies
            .get_mut(handle)
            .ok_or_else(|| RapierPhysics2DError::new("physics body handle is stale"))
    }

    fn body_id_for_collider(&self, collider: ColliderHandle) -> Option<PhysicsBodyId> {
        let parent = self.world.colliders.get(collider)?.parent()?;
        self.handles
            .iter()
            .find_map(|(id, handle)| (*handle == parent).then_some(*id))
    }

    fn query_filter(&self, filter: PhysicsQueryFilter) -> QueryFilter<'_> {
        let mut result = QueryFilter::default();
        if !filter.include_triggers {
            result.flags |= QueryFilterFlags::EXCLUDE_SENSORS;
        }
        if let Some(handle) = filter
            .exclude_body
            .and_then(|body| self.handles.get(&body).copied())
        {
            result.exclude_rigid_body = Some(handle);
        }
        result
    }

    fn update_contacts(&mut self) {
        let previous = self.contacts.iter().copied().collect::<BTreeSet<_>>();
        let mut current = BTreeSet::new();
        for pair in self
            .world
            .contact_pairs()
            .filter(|pair| pair.has_any_active_contact())
        {
            if let Some(pair) =
                self.contact_pair(pair.collider1, pair.collider2, ContactKind::Contact)
            {
                current.insert(pair);
            }
        }
        for (first, _, second, _, intersecting) in self.world.intersection_pairs() {
            if intersecting {
                if let Some(pair) = self.contact_pair(first, second, ContactKind::Trigger) {
                    current.insert(pair);
                }
            }
        }
        self.contact_events
            .extend(
                current
                    .difference(&previous)
                    .copied()
                    .map(|pair| ContactEvent {
                        transition: ContactTransition::Started,
                        pair,
                    }),
            );
        self.contact_events
            .extend(
                previous
                    .difference(&current)
                    .copied()
                    .map(|pair| ContactEvent {
                        transition: ContactTransition::Stopped,
                        pair,
                    }),
            );
        self.contacts = current.into_iter().collect();
    }

    fn contact_pair(
        &self,
        first: ColliderHandle,
        second: ColliderHandle,
        kind: ContactKind,
    ) -> Option<ContactPair> {
        let first = self.body_id_for_collider(first)?;
        let second = self.body_id_for_collider(second)?;
        (first != second).then(|| ContactPair::new(first, second, kind))
    }

    fn refresh_queries(&mut self) {
        self.world
            .bodies
            .propagate_modified_body_positions_to_colliders(&mut self.world.colliders);
        let bounds = self
            .world
            .colliders
            .iter_enabled()
            .map(|(handle, collider)| (handle, collider.compute_aabb()))
            .collect::<Vec<_>>();
        for (handle, bounds) in bounds {
            self.world
                .broad_phase
                .set_aabb(&self.world.integration_parameters, handle, bounds);
        }
    }
}

impl PhysicsProvider2D for RapierPhysicsWorld2D {
    type Error = RapierPhysics2DError;

    fn insert_body(&mut self, body: PhysicsBody2D) -> Result<(), Self::Error> {
        validate_body(&body)?;
        if self.handles.contains_key(&body.id) {
            return Err(RapierPhysics2DError::new(
                "physics body identity is already present",
            ));
        }
        let builder = match body.kind {
            PhysicsBodyKind::Dynamic => RigidBodyBuilder::dynamic(),
            PhysicsBodyKind::Fixed => RigidBodyBuilder::fixed(),
            PhysicsBodyKind::Kinematic => RigidBodyBuilder::kinematic_position_based(),
        }
        .pose(isometry(body.pose))
        .linvel(vector(body.velocity.linear))
        .angvel(body.velocity.angular_radians_per_second)
        .gravity_scale(body.gravity_scale)
        .linear_damping(body.linear_damping)
        .angular_damping(body.angular_damping)
        .ccd_enabled(body.continuous_collision_detection);
        let handle = self.world.insert_body(builder);
        for collider in body.colliders.iter().cloned() {
            self.world
                .insert_collider(collider_builder(collider), Some(handle));
        }
        self.handles.insert(body.id, handle);
        self.declarations.insert(body.id, body);
        self.refresh_queries();
        Ok(())
    }

    fn remove_body(&mut self, id: PhysicsBodyId) -> bool {
        self.declarations.remove(&id);
        self.handles
            .remove(&id)
            .and_then(|handle| self.world.remove_body(handle))
            .is_some()
    }

    fn contains_body(&self, id: PhysicsBodyId) -> bool {
        self.handles.contains_key(&id)
    }

    fn body_pose(&self, id: PhysicsBodyId) -> Option<PhysicsPose2D> {
        let body = self.world.bodies.get(*self.handles.get(&id)?)?;
        Some(PhysicsPose2D::new(
            from_vector(body.translation()),
            body.rotation().angle(),
        ))
    }

    fn body_velocity(&self, id: PhysicsBodyId) -> Option<PhysicsVelocity2D> {
        let body = self.world.bodies.get(*self.handles.get(&id)?)?;
        Some(PhysicsVelocity2D::new(
            from_vector(body.linvel()),
            body.angvel(),
        ))
    }

    fn set_body_pose(
        &mut self,
        id: PhysicsBodyId,
        pose: PhysicsPose2D,
        wake: bool,
    ) -> Result<(), Self::Error> {
        if !pose.is_valid() {
            return Err(RapierPhysics2DError::new("physics pose must be finite"));
        }
        self.body_mut(id)?.set_position(isometry(pose), wake);
        if let Some(body) = self.declarations.get_mut(&id) {
            body.pose = pose;
        }
        self.refresh_queries();
        Ok(())
    }

    fn set_body_velocity(
        &mut self,
        id: PhysicsBodyId,
        velocity: PhysicsVelocity2D,
        wake: bool,
    ) -> Result<(), Self::Error> {
        if !velocity.is_valid() {
            return Err(RapierPhysics2DError::new("physics velocity must be finite"));
        }
        let body = self.body_mut(id)?;
        body.set_linvel(vector(velocity.linear), wake);
        body.set_angvel(velocity.angular_radians_per_second, wake);
        if let Some(body) = self.declarations.get_mut(&id) {
            body.velocity = velocity;
        }
        Ok(())
    }

    fn add_force(&mut self, id: PhysicsBodyId, force: Vec2, wake: bool) -> Result<(), Self::Error> {
        if !force.is_finite() {
            return Err(RapierPhysics2DError::new("physics force must be finite"));
        }
        self.body_mut(id)?.add_force(vector(force), wake);
        Ok(())
    }

    fn apply_impulse(
        &mut self,
        id: PhysicsBodyId,
        impulse: Vec2,
        wake: bool,
    ) -> Result<(), Self::Error> {
        if !impulse.is_finite() {
            return Err(RapierPhysics2DError::new("physics impulse must be finite"));
        }
        self.body_mut(id)?.apply_impulse(vector(impulse), wake);
        Ok(())
    }

    fn cast_ray(
        &self,
        origin: Vec2,
        direction: Vec2,
        max_distance: f32,
        filter: PhysicsQueryFilter,
    ) -> Result<Option<PhysicsRayHit2D>, Self::Error> {
        let (origin, direction) = validated_ray(origin, direction, max_distance)?;
        let ray = Ray::new(origin, direction);
        let Some((collider, intersection)) =
            self.world
                .cast_ray_and_get_normal(&ray, max_distance, true, self.query_filter(filter))
        else {
            return Ok(None);
        };
        let body = self.body_id_for_collider(collider).ok_or_else(|| {
            RapierPhysics2DError::new("physics ray hit an unknown or unattached body")
        })?;
        Ok(Some(PhysicsRayHit2D {
            body,
            distance: intersection.time_of_impact,
            point: from_vector(ray.point_at(intersection.time_of_impact)),
            normal: from_vector(intersection.normal),
        }))
    }

    fn overlap_shape(
        &self,
        shape: &PhysicsShape2D,
        pose: PhysicsPose2D,
        filter: PhysicsQueryFilter,
    ) -> Result<Vec<PhysicsBodyId>, Self::Error> {
        validate_shape(shape)?;
        if !pose.is_valid() {
            return Err(RapierPhysics2DError::new("physics pose must be finite"));
        }
        let shape = shared_shape(shape);
        let bodies = self
            .world
            .intersect_shape(isometry(pose), shape.as_ref(), self.query_filter(filter))
            .filter_map(|(handle, _)| self.body_id_for_collider(handle))
            .collect::<BTreeSet<_>>();
        Ok(bodies.into_iter().collect())
    }

    fn contacts(&self) -> &[ContactPair] {
        &self.contacts
    }

    fn drain_contact_events(&mut self) -> Vec<ContactEvent> {
        std::mem::take(&mut self.contact_events)
    }

    fn step(&mut self, duration: StepDuration) {
        self.world.integration_parameters.dt = duration.as_secs_f32();
        self.world.step();
        self.update_contacts();
    }

    fn snapshot(&self) -> PhysicsSnapshot2D {
        let bodies = self
            .declarations
            .iter()
            .filter_map(|(id, declaration)| {
                let handle = self.handles.get(id)?;
                let rigid_body = self.world.bodies.get(*handle)?;
                let mut body = declaration.clone();
                body.pose = self.body_pose(*id)?;
                body.velocity = self.body_velocity(*id)?;
                Some(PhysicsBodyState2D {
                    body,
                    sleeping: rigid_body.is_sleeping(),
                })
            })
            .collect();
        PhysicsSnapshot2D {
            version: PHYSICS_SNAPSHOT_VERSION,
            gravity: self.gravity,
            bodies,
            contacts: self.contacts.clone(),
        }
    }

    fn restore(&mut self, snapshot: PhysicsSnapshot2D) -> Result<(), Self::Error> {
        if snapshot.version != PHYSICS_SNAPSHOT_VERSION {
            return Err(RapierPhysics2DError::new(format!(
                "unsupported physics snapshot version {}",
                snapshot.version
            )));
        }
        let mut restored = Self::new(snapshot.gravity)?;
        for state in snapshot.bodies {
            let id = state.body.id;
            restored.insert_body(state.body)?;
            if state.sleeping {
                restored.body_mut(id)?.sleep();
            }
        }
        restored.contacts = snapshot.contacts;
        *self = restored;
        Ok(())
    }
}

fn validate_body(body: &PhysicsBody2D) -> Result<(), RapierPhysics2DError> {
    if !body.pose.is_valid()
        || !body.velocity.is_valid()
        || !body.gravity_scale.is_finite()
        || !body.linear_damping.is_finite()
        || !body.angular_damping.is_finite()
        || body.linear_damping < 0.0
        || body.angular_damping < 0.0
        || body.colliders.is_empty()
    {
        return Err(RapierPhysics2DError::new(
            "physics body values must be finite and valid",
        ));
    }
    for collider in &body.colliders {
        if !collider.offset.is_valid()
            || !collider.density.is_finite()
            || !collider.friction.is_finite()
            || !collider.restitution.is_finite()
            || collider.density < 0.0
            || collider.friction < 0.0
            || collider.restitution < 0.0
        {
            return Err(RapierPhysics2DError::new(
                "physics collider values must be finite and nonnegative",
            ));
        }
        validate_shape(&collider.shape)?;
    }
    Ok(())
}

fn validate_shape(shape: &PhysicsShape2D) -> Result<(), RapierPhysics2DError> {
    let valid = match shape {
        PhysicsShape2D::Circle { radius } => radius.is_finite() && *radius > 0.0,
        PhysicsShape2D::Cuboid { half_extents } => {
            half_extents.is_finite() && half_extents.x > 0.0 && half_extents.y > 0.0
        }
        PhysicsShape2D::CapsuleY {
            half_height,
            radius,
        } => half_height.is_finite() && *half_height >= 0.0 && radius.is_finite() && *radius > 0.0,
    };
    valid
        .then_some(())
        .ok_or_else(|| RapierPhysics2DError::new("physics shape dimensions must be positive"))
}

fn collider_builder(collider: Collider2D) -> ColliderBuilder {
    ColliderBuilder::new(shared_shape(&collider.shape))
        .position(isometry(collider.offset))
        .density(collider.density)
        .friction(collider.friction)
        .restitution(collider.restitution)
        .sensor(collider.trigger)
}

fn shared_shape(shape: &PhysicsShape2D) -> SharedShape {
    match shape {
        PhysicsShape2D::Circle { radius } => SharedShape::ball(*radius),
        PhysicsShape2D::Cuboid { half_extents } => {
            SharedShape::cuboid(half_extents.x, half_extents.y)
        }
        PhysicsShape2D::CapsuleY {
            half_height,
            radius,
        } => SharedShape::capsule_y(*half_height, *radius),
    }
}

fn isometry(pose: PhysicsPose2D) -> Pose {
    Pose::from_parts(
        vector(pose.translation),
        Rotation::new(pose.rotation_radians),
    )
}

fn vector(value: Vec2) -> Vector {
    Vector::new(value.x, value.y)
}

fn from_vector(value: Vector) -> Vec2 {
    Vec2::new(value.x, value.y)
}

fn validated_ray(
    origin: Vec2,
    direction: Vec2,
    max_distance: f32,
) -> Result<(Vector, Vector), RapierPhysics2DError> {
    let origin = vector(origin);
    let direction = vector(direction);
    if !origin.is_finite()
        || !direction.is_finite()
        || direction.length_squared() <= f32::EPSILON
        || !max_distance.is_finite()
        || max_distance < 0.0
    {
        return Err(RapierPhysics2DError::new(
            "physics ray must have a finite origin, direction, and nonnegative distance",
        ));
    }
    Ok((origin, direction.normalize()))
}

#[cfg(test)]
mod tests {
    use super::*;

    const STEP: StepDuration = StepDuration::from_nanos(16_666_666);

    #[test]
    fn supports_fixed_dynamic_queries_transitions_and_snapshot_restore() {
        let mut world = RapierPhysicsWorld2D::new(Vec2::new(0.0, -9.81)).unwrap();
        let floor = PhysicsBodyId::new(1);
        let actor = PhysicsBodyId::new(2);
        world
            .insert_body(PhysicsBody2D::fixed(
                floor,
                PhysicsShape2D::Cuboid {
                    half_extents: Vec2::new(5.0, 0.5),
                },
            ))
            .unwrap();
        let mut dynamic = PhysicsBody2D::dynamic(actor, PhysicsShape2D::Circle { radius: 0.5 });
        dynamic.pose.translation = Vec2::new(0.0, 0.75);
        world.insert_body(dynamic).unwrap();
        assert_eq!(
            world
                .overlap_shape(
                    &PhysicsShape2D::Circle { radius: 1.0 },
                    PhysicsPose2D::new(Vec2::new(0.0, 0.75), 0.0),
                    PhysicsQueryFilter::ALL,
                )
                .unwrap(),
            vec![floor, actor]
        );
        world.step(STEP);
        assert!(!world.contacts().is_empty());
        assert_eq!(
            world.drain_contact_events()[0].transition,
            ContactTransition::Started
        );
        assert_eq!(
            world
                .overlap_shape(
                    &PhysicsShape2D::Circle { radius: 1.0 },
                    PhysicsPose2D::new(Vec2::new(0.0, 0.75), 0.0),
                    PhysicsQueryFilter::ALL,
                )
                .unwrap(),
            vec![floor, actor]
        );
        let snapshot = world.snapshot();
        world
            .apply_impulse(actor, Vec2::new(3.0, 0.0), true)
            .unwrap();
        world.step(STEP);
        world.restore(snapshot.clone()).unwrap();
        assert_eq!(world.snapshot(), snapshot);
        assert_eq!(
            world
                .cast_ray(
                    Vec2::new(0.0, 5.0),
                    Vec2::new(0.0, -1.0),
                    10.0,
                    PhysicsQueryFilter::ALL,
                )
                .unwrap()
                .unwrap()
                .body,
            actor
        );
    }

    #[test]
    fn trigger_transitions_are_distinct_from_contacts() {
        let mut world = RapierPhysicsWorld2D::new(Vec2::ZERO).unwrap();
        let zone = PhysicsBodyId::new(3);
        let actor = PhysicsBodyId::new(4);
        let mut trigger = PhysicsBody2D::fixed(zone, PhysicsShape2D::Circle { radius: 2.0 });
        trigger.colliders[0].trigger = true;
        world.insert_body(trigger).unwrap();
        world
            .insert_body(PhysicsBody2D::dynamic(
                actor,
                PhysicsShape2D::Circle { radius: 0.5 },
            ))
            .unwrap();
        world.step(STEP);
        assert_eq!(
            world.contacts(),
            &[ContactPair::new(zone, actor, ContactKind::Trigger)]
        );
    }
}
