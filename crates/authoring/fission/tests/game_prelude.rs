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
    let _: Widget = Scene2DView::new(scene, 320.0, 180.0).into();
    let _: InputTrigger = InputTrigger::Confirm;
}
