use std::collections::HashMap;
use std::fmt;
use std::time::Duration;

use fission::core::{ResourceKey, TimerResource};
use fission::game::{Game, GameHostInput, GameInputRegion, GameRuntime, GameTime, StepDuration};
use fission::i18n::{Locale, TranslationBundle};
use fission::prelude::*;
use fission::scene3d::{RenderCapabilities3D, Scene3D, Scene3DIR, Scene3DProcessor, Vec2};

use crate::game::{Direction, HarborGame, HarborMessage, BEACON_NODE};

pub struct QualificationState {
    runtime: GameRuntime<HarborGame>,
    scene: Scene3DIR,
}

impl QualificationState {
    fn new() -> Self {
        let game = HarborGame::new();
        let scene = game.present(GameTime::default());
        Self {
            runtime: GameRuntime::new(game),
            scene,
        }
    }

    fn dispatch(&mut self, message: HarborMessage) {
        self.runtime.send(message);
        self.advance();
    }

    fn advance(&mut self) {
        self.scene = self
            .runtime
            .advance(StepDuration::from_hz(60).as_duration())
            .presentation;
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

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
struct FrameTick;

#[fission_reducer(SimulateFrame)]
fn simulate_frame(state: &mut QualificationState) {
    state.advance();
}

#[fission_reducer(MoveForward)]
fn move_forward(state: &mut QualificationState) {
    state.dispatch(HarborMessage::Move(Direction::Forward));
}

#[fission_reducer(MoveBack)]
fn move_back(state: &mut QualificationState) {
    state.dispatch(HarborMessage::Move(Direction::Back));
}

#[fission_reducer(MoveLeft)]
fn move_left(state: &mut QualificationState) {
    state.dispatch(HarborMessage::Move(Direction::Left));
}

#[fission_reducer(MoveRight)]
fn move_right(state: &mut QualificationState) {
    state.dispatch(HarborMessage::Move(Direction::Right));
}

#[fission_reducer(SelectBeacon)]
fn select_beacon(state: &mut QualificationState) {
    state.dispatch(HarborMessage::SelectBeacon);
}

#[fission_reducer(RestartRun)]
fn restart_run(state: &mut QualificationState) {
    state.dispatch(HarborMessage::Restart);
}

#[fission_reducer(HandleGameInput)]
fn handle_game_input(state: &mut QualificationState, action: GameHostInput) {
    action.apply(&mut state.runtime);
    state.advance();
}

#[fission_reducer(PickViewport)]
fn pick_viewport(state: &mut QualificationState, ctx: &mut ReducerContext<QualificationState>) {
    let Some(interaction) = ctx.input.scene_interaction() else {
        return;
    };
    let mut processor = Scene3DProcessor::new();
    let prepared = processor.prepare(&state.scene, RenderCapabilities3D::default());
    let hit = prepared.pick_viewport(Vec2::new(
        interaction.viewport_point.x,
        interaction.viewport_point.y,
    ));
    if hit.is_some_and(|hit| hit.node == BEACON_NODE) {
        state.dispatch(HarborMessage::SelectBeacon);
    }
}

#[derive(Clone)]
pub struct QualificationApp;

impl From<QualificationApp> for Widget {
    fn from(_app: QualificationApp) -> Self {
        let (ctx, view) = fission::build::current::<QualificationState>();
        let state = view.state();
        let tick = with_reducer!(ctx, SimulateFrame, simulate_frame);
        let forward = with_reducer!(ctx, MoveForward, move_forward);
        let back = with_reducer!(ctx, MoveBack, move_back);
        let left = with_reducer!(ctx, MoveLeft, move_left);
        let right = with_reducer!(ctx, MoveRight, move_right);
        let select = with_reducer!(ctx, SelectBeacon, select_beacon);
        let restart = with_reducer!(ctx, RestartRun, restart_run);
        let pick = with_reducer!(ctx, PickViewport, pick_viewport);
        let host_input = with_reducer!(ctx, HandleGameInput, handle_game_input);

        ctx.with_resources(|resources| {
            resources.timer(
                TimerResource::new(
                    ResourceKey::new("scene3d-qualification-clock"),
                    Duration::from_millis(16),
                    FrameTick,
                )
                .on_tick(tick),
            );
        });

        let status_key = if state.runtime.state().success() {
            "game.success"
        } else if state.runtime.state().beacon_selected() {
            "game.selected"
        } else {
            "game.ready"
        };
        let controls = Row {
            gap: Some(8.0),
            children: vec![
                ControlButton::new("game.left", "move-left", left).into(),
                ControlButton::new("game.forward", "move-forward", forward).into(),
                ControlButton::new("game.back", "move-back", back).into(),
                ControlButton::new("game.right", "move-right", right).into(),
            ],
            ..Default::default()
        };
        let actions = Row {
            gap: Some(8.0),
            children: vec![
                ControlButton::new("game.select", "select-beacon", select).into(),
                ControlButton::new("game.restart", "restart-run", restart).into(),
            ],
            ..Default::default()
        };
        let viewport: Widget = Scene3D::new(state.scene.clone())
            .height(540.0)
            .on_pick(pick)
            .semantic_label("Interactive Beacon Run viewport")
            .into();

        let content: Widget = Container::new(Column {
            gap: Some(14.0),
            children: vec![
                Text::new(TextContent::Key("game.title".into()))
                    .size(30.0)
                    .into(),
                Text::new(TextContent::Key("game.instructions".into()))
                    .size(15.0)
                    .into(),
                Text::new(TextContent::Key(status_key.into()))
                    .size(18.0)
                    .into(),
                Container::new(viewport).height(540.0).into(),
                controls.into(),
                actions.into(),
            ],
            ..Default::default()
        })
        .padding_all(24.0)
        .into();
        GameInputRegion::for_game::<HarborGame>(content, host_input)
            .semantics_identifier("scene3d-qualification.game-input")
            .into()
    }
}

struct ControlButton {
    label: &'static str,
    identifier: &'static str,
    action: ActionEnvelope,
}

impl ControlButton {
    fn new(label: &'static str, identifier: &'static str, action: ActionEnvelope) -> Self {
        Self {
            label,
            identifier,
            action,
        }
    }
}

impl From<ControlButton> for Widget {
    fn from(control: ControlButton) -> Self {
        Button {
            child: Some(Text::new(TextContent::Key(control.label.into())).into()),
            on_press: Some(control.action),
            ..Default::default()
        }
        .semantics_identifier(control.identifier)
        .into()
    }
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
