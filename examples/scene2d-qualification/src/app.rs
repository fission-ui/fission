use std::collections::HashMap;
use std::fmt;

use fission::game::{Game, GameHostInput, GameInputRegion, GameRuntime, GameTime, StepDuration};
use fission::i18n::{Locale, TranslationBundle};
use fission::prelude::*;
use fission::scene2d::{Scene2D, Scene2DIR};

use crate::game::{
    Direction, GameMessage, MovePlayer, QualificationGame, ResetGame, VIEWPORT_SIZE,
};

pub(crate) struct QualificationState {
    runtime: GameRuntime<QualificationGame>,
    scene: Scene2DIR,
}

impl QualificationState {
    fn new() -> Self {
        let game = QualificationGame::default();
        let scene = game.present(GameTime::default());
        Self {
            runtime: GameRuntime::new(game),
            scene,
        }
    }

    fn advance(&mut self) {
        self.scene = self
            .runtime
            .advance(StepDuration::from_hz(60).as_duration())
            .presentation;
    }

    fn dispatch(&mut self, message: GameMessage) {
        self.runtime.send(message);
        self.advance();
    }
}

impl Default for QualificationState {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for QualificationState {
    fn clone(&self) -> Self {
        Self {
            runtime: GameRuntime::from_snapshot(self.runtime.snapshot())
                .expect("the runtime must restore its own snapshot"),
            scene: self.scene.clone(),
        }
    }
}

impl fmt::Debug for QualificationState {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("QualificationState")
            .field("game", self.runtime.state())
            .finish_non_exhaustive()
    }
}

impl GlobalState for QualificationState {}

fn move_player(
    state: &mut QualificationState,
    action: MovePlayer,
    _ctx: &mut ReducerContext<QualificationState>,
) {
    state.dispatch(GameMessage::Move(action.0));
}

fn reset_game(
    state: &mut QualificationState,
    _action: ResetGame,
    _ctx: &mut ReducerContext<QualificationState>,
) {
    *state = QualificationState::default();
}

fn host_input(
    state: &mut QualificationState,
    action: GameHostInput,
    _ctx: &mut ReducerContext<QualificationState>,
) {
    action.apply(&mut state.runtime);
    state.advance();
}

fission_component! {
    pub QualificationApp<QualificationState>
    |_app, state, cx| {
        let tokens = &cx.view.env().theme.tokens;
        let move_action = cx.build.bind(
            MovePlayer(Direction::Up),
            move_player
                as fn(&mut QualificationState, MovePlayer, &mut ReducerContext<QualificationState>),
        );
        let reset = cx.build.bind(
            ResetGame,
            reset_game
                as fn(&mut QualificationState, ResetGame, &mut ReducerContext<QualificationState>),
        );
        let host_input = cx.build.bind(
            GameHostInput::FocusLost,
            host_input
                as fn(
                    &mut QualificationState,
                    GameHostInput,
                    &mut ReducerContext<QualificationState>,
                ),
        );
        let game = state.runtime.state();
        let scene = Scene2D::new(state.scene.clone())
            .width(VIEWPORT_SIZE.x)
            .height(VIEWPORT_SIZE.y);
    } =>
    QualificationGameRegion(
        Container(
            Column(
                gap: tokens.spacing.m,
                children: [
                    Text(TextContent::Key("game.title".into()))
                        .size(tokens.typography.heading1_size)
                        .color(tokens.colors.text_primary),
                    if let Some(error) = &game.last_error {
                        Text(error.clone()).size(tokens.typography.body_medium_size).color(tokens.colors.error)
                    } else if game.beacon_collected {
                        Text(TextContent::Key("game.success".into())).size(tokens.typography.body_medium_size).color(tokens.colors.success)
                    } else {
                        Text(TextContent::Key("game.ready".into())).size(tokens.typography.body_medium_size).color(tokens.colors.text_secondary)
                    },
                    SemanticsRegion(
                        Container(scene)
                            .width(VIEWPORT_SIZE.x)
                            .height(VIEWPORT_SIZE.y)
                            .border(tokens.colors.border, 1.0)
                            .clip_overflow(true),
                    )
                    .identifier("scene2d-qualification.viewport"),
                    Row(
                        gap: tokens.spacing.s,
                        children: [
                            DirectionButton("←", Direction::Left, move_action.clone()),
                            DirectionButton("↑", Direction::Up, move_action.clone()),
                            DirectionButton("↓", Direction::Down, move_action.clone()),
                            DirectionButton("→", Direction::Right, move_action.clone()),
                            Button(
                                child: Text(TextContent::Key("game.restart".into())),
                                on_press: reset,
                            )
                            .semantics_identifier("beacon-run.restart"),
                        ],
                    ),
                    Text(TextContent::Key("game.controls_help".into()))
                        .size(tokens.typography.font_size_sm)
                        .color(tokens.colors.text_muted),
                ],
            ),
        )
        .padding_all(tokens.spacing.xl)
        .bg(tokens.colors.background),
        host_input,
    )
}

fission_component! {
    QualificationGameRegion<QualificationState> {
        child: Widget,
        action: ActionEnvelope,
    } |region, _state, _cx| {} =>
        GameInputRegion::for_game::<QualificationGame>(region.child.clone(), region.action.clone())
            .semantics_identifier("beacon-run.game-input")
}

pub(crate) fn create_env() -> anyhow::Result<Env> {
    let mut env = Env::default();
    for (locale, yaml) in [
        ("en-US", include_str!("../i18n/en-US.yaml")),
        ("es-ES", include_str!("../i18n/es-ES.yaml")),
    ] {
        env.i18n.add_bundle(TranslationBundle {
            locale: Locale::from(locale),
            messages: serde_yaml::from_str::<HashMap<String, String>>(yaml)?,
        });
    }
    env.locale = Locale::from("en-US");
    Ok(env)
}

fission_component! {
    DirectionButton<()> {
        label: &'static str,
        direction: Direction,
        action: ActionEnvelope,
    } |button, _state, _cx| {} =>
    Button(
        on_press: button.action.with_action(&MovePlayer(button.direction)),
        child: Text(button.label).size(20.0_f32),
    )
    .semantics_identifier(format!("beacon-run.move-{:?}", button.direction).to_lowercase())
}
