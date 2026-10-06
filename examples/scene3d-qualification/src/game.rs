use std::fmt;

use fission::game::*;
use fission::physics::*;
use fission::physics_rapier3d::RapierPhysicsWorld3D;
use fission::scene3d::{NodeId, Scene3DIR, Vec3};
use serde::{Deserialize, Serialize};

use crate::scene::build_scene;

pub const PLAYER_BODY: PhysicsBodyId = PhysicsBodyId::new(1);
pub const CARGO_BODY: PhysicsBodyId = PhysicsBodyId::new(2);
const FLOOR_BODY: PhysicsBodyId = PhysicsBodyId::new(10);
const CARGO_BLOCK_BODY: PhysicsBodyId = PhysicsBodyId::new(11);
const LEFT_WALL_BODY: PhysicsBodyId = PhysicsBodyId::new(12);
const RIGHT_WALL_BODY: PhysicsBodyId = PhysicsBodyId::new(13);
pub const BEACON_NODE: NodeId = NodeId::new(900);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Direction {
    Forward,
    Back,
    Left,
    Right,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum HarborMessage {
    Move(Direction),
    SelectBeacon,
    Restart,
}

/// Authoritative game state. Rapier stays a replaceable provider behind the
/// public Fission physics contract; cloning takes a provider-neutral snapshot.
pub struct HarborGame {
    player_position: Vec3,
    cargo_position: Vec3,
    physics: RapierPhysicsWorld3D,
    queued_move: Vec3,
    beacon_selected: bool,
    success: bool,
}

impl HarborGame {
    pub fn new() -> Self {
        let physics = new_world();
        Self {
            player_position: body_position(&physics, PLAYER_BODY),
            cargo_position: body_position(&physics, CARGO_BODY),
            physics,
            queued_move: Vec3::ZERO,
            beacon_selected: false,
            success: false,
        }
    }

    pub fn player_position(&self) -> Vec3 {
        self.player_position
    }

    pub fn cargo_position(&self) -> Vec3 {
        self.cargo_position
    }

    pub const fn beacon_selected(&self) -> bool {
        self.beacon_selected
    }

    pub const fn success(&self) -> bool {
        self.success
    }

    pub fn physics_snapshot(&self) -> PhysicsSnapshot3D {
        self.physics.snapshot()
    }

    fn queue_move(&mut self, direction: Direction) {
        let movement = direction_vector(direction);
        self.queued_move.x += movement.x;
        self.queued_move.z += movement.z;
    }
}

impl Default for HarborGame {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for HarborGame {
    fn clone(&self) -> Self {
        let snapshot = self.physics.snapshot();
        let mut physics = RapierPhysicsWorld3D::new(snapshot.gravity)
            .expect("a snapshot produced by the provider has valid gravity");
        physics
            .restore(snapshot)
            .expect("a provider must restore its own supported snapshot");
        Self {
            player_position: self.player_position,
            cargo_position: self.cargo_position,
            physics,
            queued_move: self.queued_move,
            beacon_selected: self.beacon_selected,
            success: self.success,
        }
    }
}

impl fmt::Debug for HarborGame {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("HarborGame")
            .field("player", &self.player_position())
            .field("cargo", &self.cargo_position())
            .field("beacon_selected", &self.beacon_selected)
            .field("success", &self.success)
            .finish()
    }
}

impl GameState for HarborGame {}

impl Game for HarborGame {
    type Message = HarborMessage;
    type Presentation = Scene3DIR;

    fn input(input: &mut InputMap<Self::Message>) {
        for (key, direction) in [
            (GameKey::ArrowUp, Direction::Forward),
            (GameKey::Character('w'), Direction::Forward),
            (GameKey::ArrowDown, Direction::Back),
            (GameKey::Character('s'), Direction::Back),
            (GameKey::ArrowLeft, Direction::Left),
            (GameKey::Character('a'), Direction::Left),
            (GameKey::ArrowRight, Direction::Right),
            (GameKey::Character('d'), Direction::Right),
        ] {
            input
                .on(InputTrigger::KeyPressed { key })
                .send(HarborMessage::Move(direction));
        }
        input
            .on(InputTrigger::KeyPressed {
                key: GameKey::Confirm,
            })
            .send(HarborMessage::SelectBeacon);
        input
            .on(InputTrigger::SceneGesture {
                node: BEACON_NODE,
                gesture: SceneGesture::Activate,
            })
            .send(HarborMessage::SelectBeacon);
    }

    fn react(&mut self, message: Self::Message, _ctx: &mut GameCtx<'_, Self>) {
        match message {
            HarborMessage::Move(direction) => self.queue_move(direction),
            HarborMessage::SelectBeacon => {
                self.beacon_selected = true;
                if !self.success {
                    let _ =
                        self.physics
                            .apply_impulse(CARGO_BODY, Vec3::new(0.0, 1.25, -0.8), true);
                }
            }
            HarborMessage::Restart => *self = Self::new(),
        }
    }

    fn step(&mut self, ctx: &mut StepCtx<'_, Self>) {
        let mut movement = self.queued_move;
        self.queued_move = Vec3::ZERO;
        for (key, direction) in [
            (GameKey::ArrowUp, Direction::Forward),
            (GameKey::Character('w'), Direction::Forward),
            (GameKey::ArrowDown, Direction::Back),
            (GameKey::Character('s'), Direction::Back),
            (GameKey::ArrowLeft, Direction::Left),
            (GameKey::Character('a'), Direction::Left),
            (GameKey::ArrowRight, Direction::Right),
            (GameKey::Character('d'), Direction::Right),
        ] {
            if ctx.input().key_pressed(&key) {
                let vector = direction_vector(direction);
                movement.x += vector.x;
                movement.z += vector.z;
            }
        }
        let length = (movement.x * movement.x + movement.z * movement.z).sqrt();
        if length > 0.0 {
            let distance = 3.5 * ctx.duration().as_secs_f32();
            let desired = Vec3::new(
                movement.x / length * distance,
                0.0,
                movement.z / length * distance,
            );
            self.physics
                .move_character(
                    PLAYER_BODY,
                    desired,
                    ctx.duration(),
                    CharacterController3D::default(),
                )
                .expect("qualification player declaration remains valid");
        }
        self.physics.step(ctx.duration());
        self.player_position = body_position(&self.physics, PLAYER_BODY);
        self.cargo_position = body_position(&self.physics, CARGO_BODY);

        let player = self.player_position();
        let dx = player.x;
        let dz = player.z + 3.55;
        self.success = self.beacon_selected && dx * dx + dz * dz <= 1.6 * 1.6;
    }

    fn present(&self, _time: GameTime) -> Self::Presentation {
        build_scene(self.player_position(), self.cargo_position(), self.success)
    }
}

fn direction_vector(direction: Direction) -> Vec3 {
    match direction {
        Direction::Forward => Vec3::new(0.0, 0.0, -1.0),
        Direction::Back => Vec3::new(0.0, 0.0, 1.0),
        Direction::Left => Vec3::new(-1.0, 0.0, 0.0),
        Direction::Right => Vec3::new(1.0, 0.0, 0.0),
    }
}

fn body_position(world: &RapierPhysicsWorld3D, body: PhysicsBodyId) -> Vec3 {
    world
        .body_pose(body)
        .expect("qualification physics body remains present")
        .translation
}

fn new_world() -> RapierPhysicsWorld3D {
    let mut world =
        RapierPhysicsWorld3D::new(Vec3::new(0.0, -9.81, 0.0)).expect("constant gravity is valid");
    let mut player = PhysicsBody3D::kinematic(
        PLAYER_BODY,
        PhysicsShape3D::CapsuleY {
            half_height: 0.38,
            radius: 0.34,
        },
    );
    player.pose.translation = Vec3::new(0.0, 0.76, 4.5);
    world.insert_body(player).expect("valid player body");

    let mut cargo = PhysicsBody3D::dynamic(CARGO_BODY, PhysicsShape3D::Sphere { radius: 0.38 });
    cargo.pose.translation = Vec3::new(2.0, 3.8, -0.8);
    cargo.colliders[0].restitution = 0.35;
    world.insert_body(cargo).expect("valid cargo body");

    insert_fixed(
        &mut world,
        FLOOR_BODY,
        Vec3::new(0.0, -0.25, 0.0),
        Vec3::new(6.0, 0.25, 7.0),
    );
    insert_fixed(
        &mut world,
        CARGO_BLOCK_BODY,
        Vec3::new(0.0, 0.75, 0.5),
        Vec3::new(0.8, 0.75, 0.8),
    );
    insert_fixed(
        &mut world,
        LEFT_WALL_BODY,
        Vec3::new(-4.8, 0.8, -0.8),
        Vec3::new(0.35, 0.8, 2.5),
    );
    insert_fixed(
        &mut world,
        RIGHT_WALL_BODY,
        Vec3::new(4.8, 0.8, -0.8),
        Vec3::new(0.35, 0.8, 2.5),
    );
    world
}

fn insert_fixed(
    world: &mut RapierPhysicsWorld3D,
    id: PhysicsBodyId,
    translation: Vec3,
    half_extents: Vec3,
) {
    let mut body = PhysicsBody3D::fixed(id, PhysicsShape3D::Cuboid { half_extents });
    body.pose.translation = translation;
    world.insert_body(body).expect("valid fixed body");
}
