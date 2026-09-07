#![cfg(feature = "game")]

use std::time::Duration;

use fission::prelude::*;

#[derive(Clone, Debug, PartialEq, Eq, StableKey)]
enum EntityId {
    Player,
    Resource(u32),
}

#[derive(Clone)]
struct Entity {
    id: EntityId,
}

#[derive(Clone, GameState)]
struct PreludeGame {
    #[game(object)]
    player: Entity,
    #[game(objects, key = id)]
    resources: Vec<Entity>,
    score: u32,
}

impl Game for PreludeGame {
    type Message = ();

    fn step(&mut self, _ctx: &mut StepCtx<'_, Self>) {}

    fn show(&self, _view: &mut GameView<Self>) {}
}

#[test]
fn game_runtime_and_scene_types_are_available_from_the_prelude() {
    let state = PreludeGame {
        player: Entity {
            id: EntityId::Player,
        },
        resources: vec![Entity {
            id: EntityId::Resource(7),
        }],
        score: 0,
    };
    assert_eq!(state.player().get(&state).id, EntityId::Player);
    assert_eq!(
        state
            .resources()
            .find(&state, &EntityId::Resource(7))
            .map(|item| item.id.clone()),
        Some(EntityId::Resource(7))
    );
    assert_eq!(*state.score().get(&state), 0);

    let mut game = GameTestHarness::new(state);
    let frame = game.advance(Duration::from_millis(17));
    let scene: Scene2DIR = frame.scene;
    let scene_object = SceneNodeId::from_key(&EntityId::Player);
    let cancel = ActionEnvelope {
        id: ActionId::from_name("game-prelude-drag-cancel"),
        payload: vec![1, 2, 3],
    };
    let _: Widget = Scene2DView::new(scene, 320.0, 180.0)
        .object_actions(
            scene_object,
            SceneObjectActions::new("Player").on_drag_cancel(cancel),
        )
        .into();

    let mut path_scene = Scene2D::new();
    path_scene.path(
        SceneNodeId::from_key(&EntityId::Resource(7)),
        "M0 8 Q16 0 32 8 L32 16 L0 16 Z",
        Bounds2D::from_top_left(Place::new(Px(4.0), Px(6.0)), Size::new(Px(32.0), Px(16.0))),
        Some(ir_op::Fill::Solid(Color::BLUE)),
        Some(ir_op::Stroke {
            fill: ir_op::Fill::Solid(Color::WHITE),
            width: 1.0,
            dash_array: None,
            line_cap: ir_op::LineCap::Round,
            line_join: ir_op::LineJoin::Round,
        }),
        Layer(2),
    );
    assert!(matches!(
        path_scene.finish(Tick(0)).commands.as_slice(),
        [Scene2DCommand::DrawPath { .. }]
    ));
    let _: InputTrigger = InputTrigger::Confirm;
}
