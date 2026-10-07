//! Backend-neutral physics contracts for Fission scenes and games.
//!
//! Applications own stable body identities and game state. A provider owns
//! transient solver handles, but those handles never cross this API boundary.
//! Rapier support lives in separate provider crates, so this crate has no
//! physics-engine dependency.

mod common;
mod dimension2;
mod dimension3;

pub use common::{
    ContactEvent, ContactKind, ContactPair, ContactTransition, PhysicsBodyId, PhysicsBodyKind,
    PhysicsQueryFilter, PHYSICS_SNAPSHOT_VERSION,
};
pub use dimension2::{
    Collider2D, PhysicsBody2D, PhysicsBodyState2D, PhysicsPose2D, PhysicsProvider2D,
    PhysicsRayHit2D, PhysicsShape2D, PhysicsSnapshot2D, PhysicsVelocity2D,
};
pub use dimension3::{
    CharacterController3D, CharacterMovement3D, Collider3D, PhysicsBody3D, PhysicsBodyState3D,
    PhysicsPose3D, PhysicsProvider3D, PhysicsRayHit3D, PhysicsShape3D, PhysicsSnapshot3D,
    PhysicsVelocity3D,
};
pub use fission_game::StepDuration;
pub use fission_scene::{NodeId, Quat, Vec2, Vec3};
