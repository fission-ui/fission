#![cfg(feature = "game")]

use std::time::Duration;

use fission::prelude::*;

#[derive(Clone)]
struct PreludeGame;

impl GameState for PreludeGame {}

impl Game for PreludeGame {
    type Message = ();

    fn step(&mut self, _ctx: &mut StepCtx<'_, Self>) {}

    fn show(&self, _view: &mut GameView<Self>) {}
}

#[test]
fn game_runtime_and_scene_types_are_available_from_the_prelude() {
    let mut game = GameTestHarness::new(PreludeGame);
    let frame = game.advance(Duration::from_millis(17));
    let scene: Scene2DIR = frame.scene;
    let _: Widget = Scene2DView::new(scene, 320.0, 180.0).into();
    let _: InputTrigger = InputTrigger::Confirm;
}
