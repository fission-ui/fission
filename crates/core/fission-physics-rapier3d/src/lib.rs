//! Rapier implementation of Fission's backend-neutral 3D physics contract.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use fission_physics::{
    CharacterController3D, CharacterMovement3D, Collider3D, ContactEvent, ContactKind, ContactPair,
    ContactTransition, PhysicsBody3D, PhysicsBodyId, PhysicsBodyKind, PhysicsBodyState3D,
    PhysicsPose3D, PhysicsProvider3D, PhysicsQueryFilter, PhysicsRayHit3D, PhysicsShape3D,
    PhysicsSnapshot3D, PhysicsVelocity3D, Quat, StepDuration, Vec3, PHYSICS_SNAPSHOT_VERSION,
};
use rapier3d::control::{CharacterLength, KinematicCharacterController};
use rapier3d::prelude::{
    ColliderBuilder, ColliderHandle, PhysicsWorld, Pose, QueryFilter, QueryFilterFlags, Ray,
    RigidBody, RigidBodyBuilder, RigidBodyHandle, Rotation, SharedShape, Vector,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RapierPhysics3DError {
    message: String,
}

impl RapierPhysics3DError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for RapierPhysics3DError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for RapierPhysics3DError {}

/// Fixed-step 3D world whose public boundary contains no Rapier handles.
pub struct RapierPhysicsWorld3D {
    gravity: Vec3,
    world: PhysicsWorld,
    handles: BTreeMap<PhysicsBodyId, RigidBodyHandle>,
    declarations: BTreeMap<PhysicsBodyId, PhysicsBody3D>,
    contacts: Vec<ContactPair>,
    contact_events: Vec<ContactEvent>,
}

impl RapierPhysicsWorld3D {
    pub fn new(gravity: Vec3) -> Result<Self, RapierPhysics3DError> {
        if !gravity.is_finite() {
            return Err(RapierPhysics3DError::new("physics gravity must be finite"));
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

    fn body_mut(&mut self, id: PhysicsBodyId) -> Result<&mut RigidBody, RapierPhysics3DError> {
        let handle = *self
            .handles
            .get(&id)
            .ok_or_else(|| RapierPhysics3DError::new("physics body identity is not present"))?;
        self.world
            .bodies
            .get_mut(handle)
            .ok_or_else(|| RapierPhysics3DError::new("physics body handle is stale"))
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

impl PhysicsProvider3D for RapierPhysicsWorld3D {
    type Error = RapierPhysics3DError;

    fn insert_body(&mut self, body: PhysicsBody3D) -> Result<(), Self::Error> {
        validate_body(&body)?;
        if self.handles.contains_key(&body.id) {
            return Err(RapierPhysics3DError::new(
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
        .angvel(vector(body.velocity.angular_axis_radians_per_second))
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

    fn body_pose(&self, id: PhysicsBodyId) -> Option<PhysicsPose3D> {
        let body = self.world.bodies.get(*self.handles.get(&id)?)?;
        let rotation = body.rotation();
        Some(PhysicsPose3D::new(
            from_vector(body.translation()),
            Quat::new(rotation.x, rotation.y, rotation.z, rotation.w),
        ))
    }

    fn body_velocity(&self, id: PhysicsBodyId) -> Option<PhysicsVelocity3D> {
        let body = self.world.bodies.get(*self.handles.get(&id)?)?;
        Some(PhysicsVelocity3D::new(
            from_vector(body.linvel()),
            from_vector(body.angvel()),
        ))
    }

    fn set_body_pose(
        &mut self,
        id: PhysicsBodyId,
        pose: PhysicsPose3D,
        wake: bool,
    ) -> Result<(), Self::Error> {
        if !pose.is_valid() {
            return Err(RapierPhysics3DError::new("physics pose must be valid"));
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
        velocity: PhysicsVelocity3D,
        wake: bool,
    ) -> Result<(), Self::Error> {
        if !velocity.is_valid() {
            return Err(RapierPhysics3DError::new("physics velocity must be finite"));
        }
        let body = self.body_mut(id)?;
        body.set_linvel(vector(velocity.linear), wake);
        body.set_angvel(vector(velocity.angular_axis_radians_per_second), wake);
        if let Some(body) = self.declarations.get_mut(&id) {
            body.velocity = velocity;
        }
        Ok(())
    }

    fn add_force(&mut self, id: PhysicsBodyId, force: Vec3, wake: bool) -> Result<(), Self::Error> {
        if !force.is_finite() {
            return Err(RapierPhysics3DError::new("physics force must be finite"));
        }
        self.body_mut(id)?.add_force(vector(force), wake);
        Ok(())
    }

    fn apply_impulse(
        &mut self,
        id: PhysicsBodyId,
        impulse: Vec3,
        wake: bool,
    ) -> Result<(), Self::Error> {
        if !impulse.is_finite() {
            return Err(RapierPhysics3DError::new("physics impulse must be finite"));
        }
        self.body_mut(id)?.apply_impulse(vector(impulse), wake);
        Ok(())
    }

    fn cast_ray(
        &self,
        origin: Vec3,
        direction: Vec3,
        max_distance: f32,
        filter: PhysicsQueryFilter,
    ) -> Result<Option<PhysicsRayHit3D>, Self::Error> {
        let (origin, direction) = validated_ray(origin, direction, max_distance)?;
        let ray = Ray::new(origin, direction);
        let Some((collider, intersection)) =
            self.world
                .cast_ray_and_get_normal(&ray, max_distance, true, self.query_filter(filter))
        else {
            return Ok(None);
        };
        let body = self.body_id_for_collider(collider).ok_or_else(|| {
            RapierPhysics3DError::new("physics ray hit an unknown or unattached body")
        })?;
        Ok(Some(PhysicsRayHit3D {
            body,
            distance: intersection.time_of_impact,
            point: from_vector(ray.point_at(intersection.time_of_impact)),
            normal: from_vector(intersection.normal),
        }))
    }

    fn overlap_shape(
        &self,
        shape: &PhysicsShape3D,
        pose: PhysicsPose3D,
        filter: PhysicsQueryFilter,
    ) -> Result<Vec<PhysicsBodyId>, Self::Error> {
        validate_shape(shape)?;
        if !pose.is_valid() {
            return Err(RapierPhysics3DError::new("physics pose must be valid"));
        }
        let shape = shared_shape(shape);
        let bodies = self
            .world
            .intersect_shape(isometry(pose), shape.as_ref(), self.query_filter(filter))
            .filter_map(|(handle, _)| self.body_id_for_collider(handle))
            .collect::<BTreeSet<_>>();
        Ok(bodies.into_iter().collect())
    }

    fn move_character(
        &mut self,
        body: PhysicsBodyId,
        desired_translation: Vec3,
        duration: StepDuration,
        controller: CharacterController3D,
    ) -> Result<CharacterMovement3D, Self::Error> {
        validate_character_controller(controller, desired_translation)?;
        let declaration = self
            .declarations
            .get(&body)
            .ok_or_else(|| RapierPhysics3DError::new("character body is not present"))?;
        if declaration.kind != PhysicsBodyKind::Kinematic {
            return Err(RapierPhysics3DError::new(
                "character controller requires a kinematic body",
            ));
        }
        let collider = declaration.colliders.first().ok_or_else(|| {
            RapierPhysics3DError::new("character body requires at least one collider")
        })?;
        let body_pose = self
            .body_pose(body)
            .ok_or_else(|| RapierPhysics3DError::new("character body handle is stale"))?;
        let character_pose = isometry(body_pose) * isometry(collider.offset);
        let character_shape = shared_shape(&collider.shape);
        let rapier_controller = KinematicCharacterController {
            up: vector(controller.up).normalize(),
            offset: CharacterLength::Absolute(controller.offset),
            slide: controller.slide,
            autostep: None,
            max_slope_climb_angle: controller.max_slope_climb_radians,
            min_slope_slide_angle: controller.min_slope_slide_radians,
            snap_to_ground: controller.snap_to_ground.map(CharacterLength::Absolute),
            ..Default::default()
        };
        let mut collision_handles = Vec::new();
        let movement = {
            let queries = self.world.query_pipeline_with_filter(
                QueryFilter::default()
                    .exclude_rigid_body(*self.handles.get(&body).expect("validated body"))
                    .exclude_sensors(),
            );
            rapier_controller.move_shape(
                duration.as_secs_f32(),
                &queries,
                character_shape.as_ref(),
                &character_pose,
                vector(desired_translation),
                |collision| collision_handles.push(collision.handle),
            )
        };
        let translation = from_vector(movement.translation);
        let mut next_pose = body_pose;
        next_pose.translation.x += translation.x;
        next_pose.translation.y += translation.y;
        next_pose.translation.z += translation.z;
        self.set_body_pose(body, next_pose, true)?;
        let collisions = collision_handles
            .into_iter()
            .filter_map(|handle| self.body_id_for_collider(handle))
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        Ok(CharacterMovement3D {
            translation,
            grounded: movement.grounded,
            sliding_down_slope: movement.is_sliding_down_slope,
            collisions,
        })
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

    fn snapshot(&self) -> PhysicsSnapshot3D {
        let bodies = self
            .declarations
            .iter()
            .filter_map(|(id, declaration)| {
                let handle = self.handles.get(id)?;
                let rigid_body = self.world.bodies.get(*handle)?;
                let mut body = declaration.clone();
                body.pose = self.body_pose(*id)?;
                body.velocity = self.body_velocity(*id)?;
                Some(PhysicsBodyState3D {
                    body,
                    sleeping: rigid_body.is_sleeping(),
                })
            })
            .collect();
        PhysicsSnapshot3D {
            version: PHYSICS_SNAPSHOT_VERSION,
            gravity: self.gravity,
            bodies,
            contacts: self.contacts.clone(),
        }
    }

    fn restore(&mut self, snapshot: PhysicsSnapshot3D) -> Result<(), Self::Error> {
        if snapshot.version != PHYSICS_SNAPSHOT_VERSION {
            return Err(RapierPhysics3DError::new(format!(
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

fn validate_character_controller(
    controller: CharacterController3D,
    desired_translation: Vec3,
) -> Result<(), RapierPhysics3DError> {
    let snap_is_valid = controller
        .snap_to_ground
        .is_none_or(|distance| distance.is_finite() && distance >= 0.0);
    if !controller.up.is_finite()
        || controller.up.x * controller.up.x
            + controller.up.y * controller.up.y
            + controller.up.z * controller.up.z
            <= f32::EPSILON
        || !controller.offset.is_finite()
        || controller.offset <= 0.0
        || !controller.max_slope_climb_radians.is_finite()
        || !controller.min_slope_slide_radians.is_finite()
        || !snap_is_valid
        || !desired_translation.is_finite()
    {
        return Err(RapierPhysics3DError::new(
            "character controller values must be finite and valid",
        ));
    }
    Ok(())
}

fn validate_body(body: &PhysicsBody3D) -> Result<(), RapierPhysics3DError> {
    if !body.pose.is_valid()
        || !body.velocity.is_valid()
        || !body.gravity_scale.is_finite()
        || !body.linear_damping.is_finite()
        || !body.angular_damping.is_finite()
        || body.linear_damping < 0.0
        || body.angular_damping < 0.0
        || body.colliders.is_empty()
    {
        return Err(RapierPhysics3DError::new(
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
            return Err(RapierPhysics3DError::new(
                "physics collider values must be finite and nonnegative",
            ));
        }
        validate_shape(&collider.shape)?;
    }
    Ok(())
}

fn validate_shape(shape: &PhysicsShape3D) -> Result<(), RapierPhysics3DError> {
    let valid = match shape {
        PhysicsShape3D::Sphere { radius } => radius.is_finite() && *radius > 0.0,
        PhysicsShape3D::Cuboid { half_extents } => {
            half_extents.is_finite()
                && half_extents.x > 0.0
                && half_extents.y > 0.0
                && half_extents.z > 0.0
        }
        PhysicsShape3D::CapsuleY {
            half_height,
            radius,
        } => half_height.is_finite() && *half_height >= 0.0 && radius.is_finite() && *radius > 0.0,
    };
    valid
        .then_some(())
        .ok_or_else(|| RapierPhysics3DError::new("physics shape dimensions must be positive"))
}

fn collider_builder(collider: Collider3D) -> ColliderBuilder {
    ColliderBuilder::new(shared_shape(&collider.shape))
        .position(isometry(collider.offset))
        .density(collider.density)
        .friction(collider.friction)
        .restitution(collider.restitution)
        .sensor(collider.trigger)
}

fn shared_shape(shape: &PhysicsShape3D) -> SharedShape {
    match shape {
        PhysicsShape3D::Sphere { radius } => SharedShape::ball(*radius),
        PhysicsShape3D::Cuboid { half_extents } => {
            SharedShape::cuboid(half_extents.x, half_extents.y, half_extents.z)
        }
        PhysicsShape3D::CapsuleY {
            half_height,
            radius,
        } => SharedShape::capsule_y(*half_height, *radius),
    }
}

fn isometry(pose: PhysicsPose3D) -> Pose {
    Pose::from_parts(
        vector(pose.translation),
        Rotation::from_xyzw(
            pose.rotation.x,
            pose.rotation.y,
            pose.rotation.z,
            pose.rotation.w,
        )
        .normalize(),
    )
}

fn vector(value: Vec3) -> Vector {
    Vector::new(value.x, value.y, value.z)
}

fn from_vector(value: Vector) -> Vec3 {
    Vec3::new(value.x, value.y, value.z)
}

fn validated_ray(
    origin: Vec3,
    direction: Vec3,
    max_distance: f32,
) -> Result<(Vector, Vector), RapierPhysics3DError> {
    let origin = vector(origin);
    let direction = vector(direction);
    if !origin.is_finite()
        || !direction.is_finite()
        || direction.length_squared() <= f32::EPSILON
        || !max_distance.is_finite()
        || max_distance < 0.0
    {
        return Err(RapierPhysics3DError::new(
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
    fn supports_queries_forces_and_snapshot_restore() {
        let mut world = RapierPhysicsWorld3D::new(Vec3::new(0.0, -9.81, 0.0)).unwrap();
        let floor = PhysicsBodyId::new(1);
        let actor = PhysicsBodyId::new(2);
        world
            .insert_body(PhysicsBody3D::fixed(
                floor,
                PhysicsShape3D::Cuboid {
                    half_extents: Vec3::new(5.0, 0.5, 5.0),
                },
            ))
            .unwrap();
        let mut dynamic = PhysicsBody3D::dynamic(actor, PhysicsShape3D::Sphere { radius: 0.5 });
        dynamic.pose.translation = Vec3::new(0.0, 0.75, 0.0);
        world.insert_body(dynamic).unwrap();
        assert_eq!(
            world
                .overlap_shape(
                    &PhysicsShape3D::Sphere { radius: 1.0 },
                    PhysicsPose3D::new(Vec3::new(0.0, 0.75, 0.0), Quat::IDENTITY),
                    PhysicsQueryFilter::ALL,
                )
                .unwrap(),
            vec![floor, actor]
        );
        world.step(STEP);
        assert!(!world.contacts().is_empty());
        assert_eq!(
            world
                .overlap_shape(
                    &PhysicsShape3D::Sphere { radius: 1.0 },
                    PhysicsPose3D::new(Vec3::new(0.0, 0.75, 0.0), Quat::IDENTITY),
                    PhysicsQueryFilter::ALL,
                )
                .unwrap(),
            vec![floor, actor]
        );
        let snapshot = world.snapshot();
        world
            .apply_impulse(actor, Vec3::new(3.0, 0.0, 0.0), true)
            .unwrap();
        world.step(STEP);
        world.restore(snapshot.clone()).unwrap();
        assert_eq!(world.snapshot(), snapshot);
    }

    #[test]
    fn character_controller_moves_and_stops_at_world_geometry() {
        let mut world = RapierPhysicsWorld3D::new(Vec3::ZERO).unwrap();
        let wall = PhysicsBodyId::new(3);
        let actor = PhysicsBodyId::new(4);
        let mut obstacle = PhysicsBody3D::fixed(
            wall,
            PhysicsShape3D::Cuboid {
                half_extents: Vec3::new(0.5, 2.0, 2.0),
            },
        );
        obstacle.pose.translation = Vec3::new(2.0, 1.0, 0.0);
        world.insert_body(obstacle).unwrap();
        let mut character = PhysicsBody3D::kinematic(
            actor,
            PhysicsShape3D::CapsuleY {
                half_height: 0.5,
                radius: 0.5,
            },
        );
        character.pose.translation = Vec3::new(0.0, 1.0, 0.0);
        world.insert_body(character).unwrap();
        world.step(STEP);
        let movement = world
            .move_character(
                actor,
                Vec3::new(4.0, 0.0, 0.0),
                STEP,
                CharacterController3D::default(),
            )
            .unwrap();
        assert!(movement.translation.x > 0.0);
        assert!(movement.translation.x < 4.0);
        assert_eq!(movement.collisions, vec![wall]);
    }
}
