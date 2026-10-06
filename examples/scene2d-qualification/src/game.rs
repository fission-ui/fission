use std::time::Duration;

use fission::game::{
    Game, GameCtx, GameKey, GameRuntime, GameState, GameTime, HostInputEvent, InputMap,
    InputTrigger, SceneGesture, StepCtx, StepDuration,
};
use fission::physics::{
    Collider2D, PhysicsBody2D, PhysicsBodyId, PhysicsPose2D, PhysicsProvider2D, PhysicsQueryFilter,
    PhysicsShape2D, PhysicsSnapshot2D, Vec2,
};
use fission::physics_rapier2d::RapierPhysicsWorld2D;
use fission::prelude::*;
use fission::scene2d::{NodeId, Scene2DIR};
use serde::{Deserialize, Serialize};

use crate::scene::{build_scene, CONTROL_DOWN, CONTROL_LEFT, CONTROL_RIGHT, CONTROL_UP};

pub const PLAYER_BODY: PhysicsBodyId = PhysicsBodyId::new(1);
pub const BEACON_BODY: PhysicsBodyId = PhysicsBodyId::new(2);
const OBSTACLE_BODY: PhysicsBodyId = PhysicsBodyId::new(3);
const WALL_TOP: PhysicsBodyId = PhysicsBodyId::new(4);
const WALL_BOTTOM: PhysicsBodyId = PhysicsBodyId::new(5);
const WALL_LEFT: PhysicsBodyId = PhysicsBodyId::new(6);
const WALL_RIGHT: PhysicsBodyId = PhysicsBodyId::new(7);

pub const WORLD_SIZE: Vec2 = Vec2::new(1_200.0, 720.0);
pub const VIEWPORT_SIZE: Vec2 = Vec2::new(640.0, 360.0);
pub const PLAYER_START: Vec2 = Vec2::new(96.0, 360.0);
pub const BEACON_POSITION: Vec2 = Vec2::new(1_056.0, 360.0);
pub const OBSTACLE_CENTER: Vec2 = Vec2::new(560.0, 360.0);
pub const PLAYER_RADIUS: f32 = 14.0;
const MOVE_DISTANCE: f32 = 24.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Direction {
    Up,
    Down,
    Left,
    Right,
}

impl Direction {
    const fn delta(self) -> Vec2 {
        match self {
            Self::Up => Vec2::new(0.0, -MOVE_DISTANCE),
            Self::Down => Vec2::new(0.0, MOVE_DISTANCE),
            Self::Left => Vec2::new(-MOVE_DISTANCE, 0.0),
            Self::Right => Vec2::new(MOVE_DISTANCE, 0.0),
        }
    }
}

#[fission_action]
pub struct MovePlayer(pub Direction);

#[fission_action]
pub struct ResetGame;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct QualificationGame {
    pub player: Vec2,
    pub beacon_collected: bool,
    pub moves: u32,
    pub physics: PhysicsSnapshot2D,
    pub last_error: Option<String>,
    pending_move: Option<Direction>,
}

impl Default for QualificationGame {
    fn default() -> Self {
        Self::new().expect("the built-in qualification physics scene is valid")
    }
}

impl QualificationGame {
    pub fn new() -> Result<Self, String> {
        let mut world = RapierPhysicsWorld2D::new(Vec2::ZERO).map_err(|error| error.to_string())?;
        let mut player = PhysicsBody2D::kinematic(
            PLAYER_BODY,
            PhysicsShape2D::Circle {
                radius: PLAYER_RADIUS,
            },
        );
        player.pose = PhysicsPose2D::new(PLAYER_START, 0.0);
        player.gravity_scale = 0.0;
        world
            .insert_body(player)
            .map_err(|error| error.to_string())?;

        let mut beacon = PhysicsBody2D::fixed(BEACON_BODY, PhysicsShape2D::Circle { radius: 24.0 });
        beacon.pose = PhysicsPose2D::new(BEACON_POSITION, 0.0);
        beacon.colliders[0].trigger = true;
        world
            .insert_body(beacon)
            .map_err(|error| error.to_string())?;

        insert_box(
            &mut world,
            OBSTACLE_BODY,
            OBSTACLE_CENTER,
            Vec2::new(92.0, 132.0),
        )?;
        insert_box(
            &mut world,
            WALL_TOP,
            Vec2::new(WORLD_SIZE.x / 2.0, -10.0),
            Vec2::new(WORLD_SIZE.x / 2.0, 10.0),
        )?;
        insert_box(
            &mut world,
            WALL_BOTTOM,
            Vec2::new(WORLD_SIZE.x / 2.0, WORLD_SIZE.y + 10.0),
            Vec2::new(WORLD_SIZE.x / 2.0, 10.0),
        )?;
        insert_box(
            &mut world,
            WALL_LEFT,
            Vec2::new(-10.0, WORLD_SIZE.y / 2.0),
            Vec2::new(10.0, WORLD_SIZE.y / 2.0),
        )?;
        insert_box(
            &mut world,
            WALL_RIGHT,
            Vec2::new(WORLD_SIZE.x + 10.0, WORLD_SIZE.y / 2.0),
            Vec2::new(10.0, WORLD_SIZE.y / 2.0),
        )?;

        Ok(Self {
            player: PLAYER_START,
            beacon_collected: false,
            moves: 0,
            physics: world.snapshot(),
            last_error: None,
            pending_move: None,
        })
    }

    pub fn run_message(&mut self, message: MovePlayer) {
        let mut runtime = GameRuntime::new(self.clone());
        runtime.send(GameMessage::Move(message.0));
        runtime.advance(StepDuration::from_hz(60).as_duration());
        *self = runtime.state().clone();
    }

    fn move_once(&mut self, direction: Direction, duration: StepDuration) -> Result<(), String> {
        let mut world = RapierPhysicsWorld2D::new(Vec2::ZERO).map_err(|error| error.to_string())?;
        world
            .restore(self.physics.clone())
            .map_err(|error| error.to_string())?;
        let delta = direction.delta();
        let candidate = Vec2::new(self.player.x + delta.x, self.player.y + delta.y);
        let shape = PhysicsShape2D::Circle {
            radius: PLAYER_RADIUS,
        };
        let solids = world
            .overlap_shape(
                &shape,
                PhysicsPose2D::new(candidate, 0.0),
                PhysicsQueryFilter::SOLIDS.excluding(PLAYER_BODY),
            )
            .map_err(|error| error.to_string())?;
        if solids.is_empty() {
            world
                .set_body_pose(PLAYER_BODY, PhysicsPose2D::new(candidate, 0.0), true)
                .map_err(|error| error.to_string())?;
            self.player = candidate;
        }
        world.step(duration);
        let overlaps = world
            .overlap_shape(
                &shape,
                PhysicsPose2D::new(self.player, 0.0),
                PhysicsQueryFilter::ALL.excluding(PLAYER_BODY),
            )
            .map_err(|error| error.to_string())?;
        if overlaps.contains(&BEACON_BODY) {
            self.beacon_collected = true;
            world.remove_body(BEACON_BODY);
        }
        self.physics = world.snapshot();
        self.moves = self.moves.saturating_add(1);
        Ok(())
    }
}

impl GlobalState for QualificationGame {}
impl GameState for QualificationGame {}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum GameMessage {
    Move(Direction),
    Reset,
}

impl Game for QualificationGame {
    type Message = GameMessage;
    type Presentation = Scene2DIR;

    fn input(input: &mut InputMap<Self::Message>) {
        for (key, direction) in [
            (GameKey::ArrowUp, Direction::Up),
            (GameKey::ArrowDown, Direction::Down),
            (GameKey::ArrowLeft, Direction::Left),
            (GameKey::ArrowRight, Direction::Right),
        ] {
            input
                .on(InputTrigger::KeyPressed { key })
                .send(GameMessage::Move(direction));
        }
        for (node, direction) in [
            (CONTROL_UP, Direction::Up),
            (CONTROL_DOWN, Direction::Down),
            (CONTROL_LEFT, Direction::Left),
            (CONTROL_RIGHT, Direction::Right),
        ] {
            input
                .on(InputTrigger::SceneGesture {
                    node: NodeId::new(node),
                    gesture: SceneGesture::Tap,
                })
                .send(GameMessage::Move(direction));
            input
                .on(InputTrigger::SceneGesture {
                    node: NodeId::new(node),
                    gesture: SceneGesture::Activate,
                })
                .send(GameMessage::Move(direction));
        }
    }

    fn react(&mut self, message: Self::Message, _ctx: &mut GameCtx<'_, Self>) {
        match message {
            GameMessage::Move(direction) => self.pending_move = Some(direction),
            GameMessage::Reset => *self = Self::default(),
        }
    }

    fn step(&mut self, ctx: &mut StepCtx<'_, Self>) {
        if let Some(direction) = self.pending_move.take() {
            if let Err(error) = self.move_once(direction, ctx.duration()) {
                self.last_error = Some(error);
            }
        }
    }

    fn present(&self, _time: GameTime) -> Self::Presentation {
        build_scene(self)
    }
}

fn insert_box(
    world: &mut RapierPhysicsWorld2D,
    id: PhysicsBodyId,
    center: Vec2,
    half_extents: Vec2,
) -> Result<(), String> {
    let mut body = PhysicsBody2D::fixed(id, PhysicsShape2D::Cuboid { half_extents });
    body.pose = PhysicsPose2D::new(center, 0.0);
    body.colliders[0] = Collider2D::new(PhysicsShape2D::Cuboid { half_extents });
    world.insert_body(body).map_err(|error| error.to_string())
}

pub fn run_step(runtime: &mut GameRuntime<QualificationGame>, message: GameMessage) {
    runtime.send(message);
    runtime.advance(Duration::from_nanos(StepDuration::from_hz(60).as_nanos()));
}

#[cfg(test)]
mod tests {
    use fission::game::{
        GameRecorder, GameReplay, GameTestHarness, PointerId, PointerKind, PointerPhase,
    };
    use fission::scene2d::Scene2DProcessor;

    use super::*;

    fn complete_run(send: &mut impl FnMut(GameMessage)) {
        for _ in 0..8 {
            send(GameMessage::Move(Direction::Up));
        }
        for _ in 0..40 {
            send(GameMessage::Move(Direction::Right));
        }
        for _ in 0..8 {
            send(GameMessage::Move(Direction::Down));
        }
    }

    #[test]
    fn obstacle_blocks_the_direct_route() {
        let mut harness = GameTestHarness::new(QualificationGame::default());
        for _ in 0..30 {
            harness.send(GameMessage::Move(Direction::Right));
            harness.tick();
        }
        assert!(harness.state().player.x < OBSTACLE_CENTER.x - 92.0);
        assert!(!harness.state().beacon_collected);
    }

    #[test]
    fn keyboard_pointer_and_accessible_activation_share_the_message_path() {
        let mut keyboard = GameTestHarness::new(QualificationGame::default());
        keyboard.input(HostInputEvent::Key {
            key: GameKey::ArrowRight,
            pressed: true,
        });
        keyboard.tick();

        let mut pointer = GameTestHarness::new(QualificationGame::default());
        let prepared = Scene2DProcessor::prepare(&pointer.state().present(GameTime::default()));
        let control = prepared
            .draws
            .iter()
            .find(|draw| draw.metadata().node == NodeId::new(CONTROL_RIGHT))
            .expect("right control is visible")
            .metadata()
            .view_bounds;
        let coordinate = Vec2::new(
            (control.min.x + control.max.x) / 2.0,
            (control.min.y + control.max.y) / 2.0,
        );
        let hit = prepared
            .pick_interactive(coordinate)
            .expect("coordinate picks the right control");
        assert_eq!(hit.node, NodeId::new(CONTROL_RIGHT));
        pointer.input(HostInputEvent::Pointer {
            id: PointerId(1),
            kind: PointerKind::Touch,
            phase: PointerPhase::Down,
            button: None,
            position: coordinate,
            target: Some(hit.node),
        });
        pointer.input(HostInputEvent::SceneGesture {
            node: hit.node,
            gesture: SceneGesture::Tap,
        });
        pointer.tick();

        let mut accessible = GameTestHarness::new(QualificationGame::default());
        accessible.input(HostInputEvent::SceneGesture {
            node: NodeId::new(CONTROL_RIGHT),
            gesture: SceneGesture::Activate,
        });
        accessible.tick();

        assert_eq!(keyboard.state().player, pointer.state().player);
        assert_eq!(pointer.state().player, accessible.state().player);
    }

    #[test]
    fn snapshot_restore_finishes_the_same_complete_run() {
        let mut runtime = GameRuntime::new(QualificationGame::default());
        for _ in 0..8 {
            run_step(&mut runtime, GameMessage::Move(Direction::Up));
        }
        for _ in 0..18 {
            run_step(&mut runtime, GameMessage::Move(Direction::Right));
        }
        let snapshot = runtime.snapshot();

        let mut expected = runtime;
        for _ in 0..22 {
            run_step(&mut expected, GameMessage::Move(Direction::Right));
        }
        for _ in 0..8 {
            run_step(&mut expected, GameMessage::Move(Direction::Down));
        }

        let mut restored = GameRuntime::from_snapshot(snapshot).expect("valid game snapshot");
        for _ in 0..22 {
            run_step(&mut restored, GameMessage::Move(Direction::Right));
        }
        for _ in 0..8 {
            run_step(&mut restored, GameMessage::Move(Direction::Down));
        }

        assert!(expected.state().beacon_collected);
        assert_eq!(restored.state(), expected.state());
    }

    #[test]
    fn recorded_complete_run_replays_to_identical_state() {
        let mut recorder = GameRecorder::new(QualificationGame::default());
        complete_run(&mut |message| {
            recorder.send(message);
            recorder.advance(StepDuration::from_hz(60).as_duration());
        });
        let expected = recorder.state().clone();
        assert!(expected.beacon_collected);

        let replay: GameReplay<QualificationGame, GameMessage> = recorder.finish();
        let replayed = GameRuntime::<QualificationGame>::replay(replay).expect("valid replay");
        assert_eq!(replayed.runtime.state(), &expected);
    }
}
