//! Deterministic application-facing game loop and semantic input mapping.

use std::collections::VecDeque;
use std::fmt::Debug;
use std::marker::PhantomData;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::{FixedStepClock, Scene2D, Scene2DIR, SceneNodeId, StepBatch, StepDuration, Tick};

/// Marker for authoritative game state managed by [`GameRuntime`].
///
/// State must be cloneable so tests, snapshots, and later replay facilities can
/// take an explicit value snapshot without borrowing runtime internals.
pub trait GameState: Clone + 'static {}

/// A logical keyboard input independent of a platform scan code.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum GameKey {
    ArrowUp,
    ArrowDown,
    ArrowLeft,
    ArrowRight,
    Confirm,
    Cancel,
    Space,
    Character(char),
}

/// A device-independent input gesture that can be bound to a game message.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "trigger", content = "value")]
pub enum InputTrigger {
    KeyPressed(GameKey),
    KeyReleased(GameKey),
    /// Activates the visible scene object with this stable identity.
    Tap(SceneNodeId),
    Confirm,
    Cancel,
}

/// Input delivered by a Fission shell to a game runtime.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "event", content = "value")]
pub enum HostInputEvent {
    Trigger(InputTrigger),
    /// Clears transient device state without manufacturing a gameplay message.
    FocusLost,
}

/// Declarative mapping from host gestures to a game's typed messages.
#[derive(Clone, Debug)]
pub struct InputMap<M> {
    bindings: Vec<(InputTrigger, M)>,
}

impl<M> Default for InputMap<M> {
    fn default() -> Self {
        Self {
            bindings: Vec::new(),
        }
    }
}

impl<M> InputMap<M> {
    pub fn new() -> Self {
        Self::default()
    }

    /// Starts a binding declaration. Calling [`InputBinding::send`] completes
    /// it and retains the message in declaration order.
    pub fn on(&mut self, trigger: InputTrigger) -> InputBinding<'_, M> {
        InputBinding { map: self, trigger }
    }
}

impl<M: Clone> InputMap<M> {
    fn messages_for<'a>(&'a self, trigger: &'a InputTrigger) -> impl Iterator<Item = M> + 'a {
        self.bindings
            .iter()
            .filter(move |(candidate, _)| candidate == trigger)
            .map(|(_, message)| message.clone())
    }
}

/// In-progress fluent input binding returned by [`InputMap::on`].
#[must_use = "complete the binding with .send(message)"]
pub struct InputBinding<'a, M> {
    map: &'a mut InputMap<M>,
    trigger: InputTrigger,
}

impl<M> InputBinding<'_, M> {
    pub fn send(self, message: M) {
        self.map.bindings.push((self.trigger, message));
    }
}

/// Deterministic fixed-step configuration for one game.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GameConfig {
    pub step: StepDuration,
    pub max_steps_per_frame: u32,
    pub max_messages_per_step: u32,
}

/// Versioned renderer-independent checkpoint for one game runtime.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GameSnapshot<S, M> {
    pub format_version: u16,
    pub game: S,
    pub pending_messages: Vec<M>,
    pub clock: crate::FixedStepClockSnapshot,
    pub config: GameConfig,
    pub completed_tick: Option<Tick>,
}

impl<S, M> GameSnapshot<S, M> {
    pub const FORMAT_VERSION: u16 = 1;
}

/// Invalid or incompatible game checkpoint.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GameSnapshotError {
    message: String,
}

impl GameSnapshotError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl std::fmt::Display for GameSnapshotError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for GameSnapshotError {}

/// One external operation captured in a deterministic game replay.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "operation", content = "value")]
pub enum GameReplayEvent<M> {
    /// Input delivered through the runtime's declarative input map.
    Input(HostInputEvent),
    /// Typed application message sent directly by the host.
    Message(M),
    /// Presentation time supplied before producing one frame.
    Advance { elapsed_nanos: u128 },
}

/// Versioned renderer-independent input and timing recording.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GameReplay<S, M> {
    pub format_version: u16,
    pub initial_snapshot: GameSnapshot<S, M>,
    pub events: Vec<GameReplayEvent<M>>,
}

impl<S, M> GameReplay<S, M> {
    pub const FORMAT_VERSION: u16 = 1;
}

/// Invalid or incompatible game replay.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GameReplayError {
    message: String,
}

impl GameReplayError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl std::fmt::Display for GameReplayError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for GameReplayError {}

impl Default for GameConfig {
    fn default() -> Self {
        Self {
            step: StepDuration::from_hz(60),
            max_steps_per_frame: 8,
            max_messages_per_step: 1_024,
        }
    }
}

impl GameConfig {
    pub fn validate(self) -> Self {
        assert!(
            self.max_steps_per_frame > 0,
            "maximum steps per frame must be non-zero"
        );
        assert!(
            self.max_messages_per_step > 0,
            "maximum messages per step must be non-zero"
        );
        self
    }
}

/// Time visible to declarative presentation after a simulation advance.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct GameTime {
    pub completed_tick: Option<Tick>,
    pub interpolation: f32,
}

/// Runtime diagnostic which does not alter authoritative game state.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RuntimeDiagnostic {
    DroppedSimulationTime(Duration),
    MessageBudgetExceeded { tick: Tick, deferred: usize },
}

/// Context for reacting to one typed input message.
pub struct GameCtx<'a, G: Game> {
    tick: Tick,
    pending: &'a mut VecDeque<G::Message>,
}

impl<G: Game> GameCtx<'_, G> {
    pub const fn tick(&self) -> Tick {
        self.tick
    }

    /// Queues a follow-up message after messages already captured for this
    /// simulation step.
    pub fn send(&mut self, message: G::Message) {
        self.pending.push_back(message);
    }
}

/// Context for one fixed simulation step.
pub struct StepCtx<'a, G: Game> {
    tick: Tick,
    duration: StepDuration,
    pending: &'a mut VecDeque<G::Message>,
}

impl<G: Game> StepCtx<'_, G> {
    pub const fn tick(&self) -> Tick {
        self.tick
    }

    pub const fn duration(&self) -> StepDuration {
        self.duration
    }

    pub fn send(&mut self, message: G::Message) {
        self.pending.push_back(message);
    }
}

/// Declarative presentation collector used by [`Game::show`].
pub struct GameView<G: Game> {
    scene: Scene2D,
    _game: PhantomData<fn() -> G>,
}

impl<G: Game> Default for GameView<G> {
    fn default() -> Self {
        Self {
            scene: Scene2D::new(),
            _game: PhantomData,
        }
    }
}

impl<G: Game> GameView<G> {
    pub fn new() -> Self {
        Self::default()
    }

    /// Exposes the expert 2D scene builder without creating a second scene or
    /// render path. Everyday `show` helpers will lower into this same authority.
    pub fn raw_scene2d(&mut self) -> &mut Scene2D {
        &mut self.scene
    }

    fn finish(self, tick: Tick) -> Scene2DIR {
        self.scene.finish(tick)
    }
}

/// A deterministic game whose state, messages, simulation, and presentation
/// remain independent of a particular renderer or shell.
pub trait Game: GameState + Sized {
    type Message: Clone + Debug + 'static;

    fn input(_input: &mut InputMap<Self::Message>) {}

    fn react(&mut self, _message: Self::Message, _ctx: &mut GameCtx<'_, Self>) {}

    fn step(&mut self, ctx: &mut StepCtx<'_, Self>);

    fn show(&self, view: &mut GameView<Self>);
}

/// One completed presentation update from [`GameRuntime::advance`].
#[derive(Clone, Debug, PartialEq)]
pub struct GameFrame {
    pub steps: StepBatch,
    pub time: GameTime,
    pub scene: Scene2DIR,
    pub diagnostics: Vec<RuntimeDiagnostic>,
}

/// Host-independent authority for typed input, fixed-step simulation, and
/// declarative scene production.
pub struct GameRuntime<G: Game> {
    game: G,
    input: InputMap<G::Message>,
    pending: VecDeque<G::Message>,
    clock: FixedStepClock,
    config: GameConfig,
    completed_tick: Option<Tick>,
}

/// Headless result of executing every operation in a [`GameReplay`].
pub struct GameReplayRun<G: Game> {
    pub runtime: GameRuntime<G>,
    pub frames: Vec<GameFrame>,
}

impl<G: Game> GameRuntime<G> {
    pub fn new(game: G) -> Self {
        Self::with_config(game, GameConfig::default())
    }

    pub fn with_config(game: G, config: GameConfig) -> Self {
        let config = config.validate();
        let mut input = InputMap::new();
        G::input(&mut input);
        Self {
            game,
            input,
            pending: VecDeque::new(),
            clock: FixedStepClock::new(config.step)
                .with_max_steps_per_frame(config.max_steps_per_frame),
            config,
            completed_tick: None,
        }
    }

    pub const fn state(&self) -> &G {
        &self.game
    }

    pub fn state_mut(&mut self) -> &mut G {
        &mut self.game
    }

    /// Maps a host event to zero or more typed messages. State changes remain
    /// fixed-step deterministic: queued messages are delivered at the next tick.
    pub fn handle_input(&mut self, event: HostInputEvent) {
        if let HostInputEvent::Trigger(trigger) = event {
            self.pending.extend(self.input.messages_for(&trigger));
        }
    }

    pub fn send(&mut self, message: G::Message) {
        self.pending.push_back(message);
    }

    /// Captures authoritative game, input queue, and sub-step clock state.
    pub fn snapshot(&self) -> GameSnapshot<G, G::Message> {
        GameSnapshot {
            format_version: GameSnapshot::<G, G::Message>::FORMAT_VERSION,
            game: self.game.clone(),
            pending_messages: self.pending.iter().cloned().collect(),
            clock: self.clock.snapshot(),
            config: self.config,
            completed_tick: self.completed_tick,
        }
    }

    /// Restores a runtime without depending on a renderer or wall clock.
    pub fn from_snapshot(snapshot: GameSnapshot<G, G::Message>) -> Result<Self, GameSnapshotError> {
        if snapshot.format_version != GameSnapshot::<G, G::Message>::FORMAT_VERSION {
            return Err(GameSnapshotError::new(
                "game snapshot format version is unsupported",
            ));
        }
        if snapshot.config.step != snapshot.clock.step
            || snapshot.config.max_steps_per_frame != snapshot.clock.max_steps_per_frame
        {
            return Err(GameSnapshotError::new(
                "game snapshot clock does not match its runtime configuration",
            ));
        }
        if snapshot.config.step.as_nanos() == 0
            || snapshot.config.max_steps_per_frame == 0
            || snapshot.config.max_messages_per_step == 0
        {
            return Err(GameSnapshotError::new(
                "game snapshot runtime configuration is invalid",
            ));
        }
        let completed_tick_matches_clock = match snapshot.completed_tick {
            None => snapshot.clock.next_tick == Tick(0),
            Some(completed) => completed.0.saturating_add(1) == snapshot.clock.next_tick.0,
        };
        if !completed_tick_matches_clock {
            return Err(GameSnapshotError::new(
                "game snapshot completed tick does not match its clock",
            ));
        }
        let config = snapshot.config;
        let clock = FixedStepClock::from_snapshot(snapshot.clock)
            .ok_or_else(|| GameSnapshotError::new("game snapshot clock is invalid"))?;
        let mut input = InputMap::new();
        G::input(&mut input);
        Ok(Self {
            game: snapshot.game,
            input,
            pending: snapshot.pending_messages.into_iter().collect(),
            clock,
            config,
            completed_tick: snapshot.completed_tick,
        })
    }

    /// Replays captured input, host messages, and frame timing from a checkpoint.
    pub fn replay(replay: GameReplay<G, G::Message>) -> Result<GameReplayRun<G>, GameReplayError> {
        if replay.format_version != GameReplay::<G, G::Message>::FORMAT_VERSION {
            return Err(GameReplayError::new(
                "game replay format version is unsupported",
            ));
        }
        let mut runtime = Self::from_snapshot(replay.initial_snapshot).map_err(|error| {
            GameReplayError::new(format!("game replay snapshot is invalid: {error}"))
        })?;
        let mut frames = Vec::new();
        for (index, event) in replay.events.into_iter().enumerate() {
            match event {
                GameReplayEvent::Input(input) => runtime.handle_input(input),
                GameReplayEvent::Message(message) => runtime.send(message),
                GameReplayEvent::Advance { elapsed_nanos } => {
                    let elapsed = duration_from_nanos(elapsed_nanos).ok_or_else(|| {
                        GameReplayError::new(format!(
                            "game replay event {index} elapsed time is out of range"
                        ))
                    })?;
                    frames.push(runtime.advance(elapsed));
                }
            }
        }
        Ok(GameReplayRun { runtime, frames })
    }

    pub fn advance(&mut self, elapsed: Duration) -> GameFrame {
        let steps = self.clock.advance(elapsed);
        let mut diagnostics = Vec::new();
        if !steps.dropped.is_zero() {
            diagnostics.push(RuntimeDiagnostic::DroppedSimulationTime(steps.dropped));
        }

        for tick in steps.ticks() {
            self.deliver_messages(tick, &mut diagnostics);
            let mut ctx = StepCtx::<G> {
                tick,
                duration: self.config.step,
                pending: &mut self.pending,
            };
            self.game.step(&mut ctx);
            self.completed_tick = Some(tick);
        }

        let time = GameTime {
            completed_tick: self.completed_tick,
            interpolation: steps.interpolation,
        };
        let mut view = GameView::new();
        self.game.show(&mut view);
        let scene = view.finish(self.completed_tick.unwrap_or(Tick(0)));

        GameFrame {
            steps,
            time,
            scene,
            diagnostics,
        }
    }

    fn deliver_messages(&mut self, tick: Tick, diagnostics: &mut Vec<RuntimeDiagnostic>) {
        let available = self.pending.len();
        let count = available.min(self.config.max_messages_per_step as usize);
        for _ in 0..count {
            let Some(message) = self.pending.pop_front() else {
                break;
            };
            let mut ctx = GameCtx::<G> {
                tick,
                pending: &mut self.pending,
            };
            self.game.react(message, &mut ctx);
        }
        if available > count {
            diagnostics.push(RuntimeDiagnostic::MessageBudgetExceeded {
                tick,
                deferred: self.pending.len(),
            });
        }
    }
}

/// Runtime wrapper that records only host-owned operations.
///
/// Messages emitted from [`GameCtx`] or [`StepCtx`] are deterministic
/// consequences of the recorded operations and are therefore not duplicated
/// in the replay stream.
pub struct GameRecorder<G: Game> {
    runtime: GameRuntime<G>,
    replay: GameReplay<G, G::Message>,
}

impl<G: Game> GameRecorder<G> {
    pub fn new(game: G) -> Self {
        Self::from_runtime(GameRuntime::new(game))
    }

    pub fn with_config(game: G, config: GameConfig) -> Self {
        Self::from_runtime(GameRuntime::with_config(game, config))
    }

    pub fn from_runtime(runtime: GameRuntime<G>) -> Self {
        let initial_snapshot = runtime.snapshot();
        Self {
            runtime,
            replay: GameReplay {
                format_version: GameReplay::<G, G::Message>::FORMAT_VERSION,
                initial_snapshot,
                events: Vec::new(),
            },
        }
    }

    pub const fn state(&self) -> &G {
        self.runtime.state()
    }

    pub fn state_mut(&mut self) -> &mut G {
        self.runtime.state_mut()
    }

    pub fn handle_input(&mut self, event: HostInputEvent) {
        self.replay
            .events
            .push(GameReplayEvent::Input(event.clone()));
        self.runtime.handle_input(event);
    }

    pub fn send(&mut self, message: G::Message) {
        self.replay
            .events
            .push(GameReplayEvent::Message(message.clone()));
        self.runtime.send(message);
    }

    pub fn advance(&mut self, elapsed: Duration) -> GameFrame {
        self.replay.events.push(GameReplayEvent::Advance {
            elapsed_nanos: elapsed.as_nanos(),
        });
        self.runtime.advance(elapsed)
    }

    pub const fn recording(&self) -> &GameReplay<G, G::Message> {
        &self.replay
    }

    pub fn into_parts(self) -> (GameRuntime<G>, GameReplay<G, G::Message>) {
        (self.runtime, self.replay)
    }
}

fn duration_from_nanos(nanos: u128) -> Option<Duration> {
    let seconds = u64::try_from(nanos / 1_000_000_000).ok()?;
    let subsecond_nanos = (nanos % 1_000_000_000) as u32;
    Some(Duration::new(seconds, subsecond_nanos))
}

/// Headless deterministic game fixture using the production runtime authority.
pub struct GameTestHarness<G: Game> {
    runtime: GameRuntime<G>,
}

impl<G: Game> GameTestHarness<G> {
    pub fn new(game: G) -> Self {
        Self {
            runtime: GameRuntime::new(game),
        }
    }

    pub fn with_config(game: G, config: GameConfig) -> Self {
        Self {
            runtime: GameRuntime::with_config(game, config),
        }
    }

    pub const fn state(&self) -> &G {
        self.runtime.state()
    }

    pub fn input(&mut self, trigger: InputTrigger) -> &mut Self {
        self.runtime.handle_input(HostInputEvent::Trigger(trigger));
        self
    }

    pub fn send(&mut self, message: G::Message) -> &mut Self {
        self.runtime.send(message);
        self
    }

    pub fn advance(&mut self, elapsed: Duration) -> GameFrame {
        self.runtime.advance(elapsed)
    }

    pub fn step(&mut self) -> GameFrame {
        self.runtime.advance(self.runtime.config.step.as_duration())
    }
}

#[cfg(test)]
mod tests {
    use fission_ir::op::Color;

    use super::*;
    use crate::{Bounds2D, Layer, Place, Px, Size};

    #[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
    struct CounterGame {
        value: i32,
        steps: Vec<Tick>,
    }

    impl GameState for CounterGame {}

    #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
    enum Message {
        Add(i32),
        AddAgain,
    }

    impl Game for CounterGame {
        type Message = Message;

        fn input(input: &mut InputMap<Self::Message>) {
            input
                .on(InputTrigger::KeyPressed(GameKey::Space))
                .send(Message::Add(1));
            input
                .on(InputTrigger::Tap(SceneNodeId::from_key(&7_u32)))
                .send(Message::Add(10));
        }

        fn react(&mut self, message: Self::Message, ctx: &mut GameCtx<'_, Self>) {
            match message {
                Message::Add(value) => self.value += value,
                Message::AddAgain => {
                    self.value += 1;
                    ctx.send(Message::Add(2));
                }
            }
        }

        fn step(&mut self, ctx: &mut StepCtx<'_, Self>) {
            self.steps.push(ctx.tick());
        }

        fn show(&self, view: &mut GameView<Self>) {
            view.raw_scene2d().rect(
                SceneNodeId::from_key(&1_u32),
                Bounds2D::from_top_left(
                    Place::new(Px(0.0), Px(0.0)),
                    Size::new(Px(self.value.max(0) as f32), Px(1.0)),
                ),
                Color::WHITE,
                Layer(0),
            );
        }
    }

    #[test]
    fn captured_input_is_delivered_in_order_at_the_next_fixed_tick() {
        let mut game = GameTestHarness::new(CounterGame::default());
        game.input(InputTrigger::KeyPressed(GameKey::Space))
            .input(InputTrigger::Tap(SceneNodeId::from_key(&7_u32)));

        let frame = game.step();
        assert_eq!(game.state().value, 11);
        assert_eq!(game.state().steps, vec![Tick(0)]);
        assert_eq!(frame.scene.tick, Tick(0));
    }

    #[test]
    fn equal_input_and_elapsed_sequences_are_deterministic() {
        let run = || {
            let mut game = GameTestHarness::new(CounterGame::default());
            game.send(Message::Add(4));
            let first = game.advance(Duration::from_millis(10));
            let second = game.advance(Duration::from_millis(40));
            (game.state().clone(), first, second)
        };
        assert_eq!(run(), run());
    }

    #[test]
    fn messages_emitted_while_reacting_keep_fifo_order_without_recursion() {
        let config = GameConfig {
            step: StepDuration::from_hz(10),
            max_steps_per_frame: 2,
            max_messages_per_step: 2,
        };
        let mut game = GameTestHarness::with_config(CounterGame::default(), config);
        game.send(Message::AddAgain).send(Message::Add(4));

        game.step();
        assert_eq!(game.state().value, 5);
        game.step();
        assert_eq!(game.state().value, 7);
    }

    #[test]
    fn serialized_snapshot_restores_pending_input_and_sub_step_time() {
        let config = GameConfig {
            step: StepDuration::from_hz(20),
            max_steps_per_frame: 4,
            max_messages_per_step: 8,
        };
        let mut original = GameRuntime::with_config(CounterGame::default(), config);
        original.handle_input(HostInputEvent::Trigger(InputTrigger::KeyPressed(
            GameKey::Space,
        )));
        original.advance(Duration::from_millis(60));
        original.handle_input(HostInputEvent::Trigger(InputTrigger::Tap(
            SceneNodeId::from_key(&7_u32),
        )));

        let encoded = serde_json::to_string(&original.snapshot()).unwrap();
        let decoded = serde_json::from_str(&encoded).unwrap();
        let mut restored = GameRuntime::<CounterGame>::from_snapshot(decoded).unwrap();

        let original_frame = original.advance(Duration::from_millis(40));
        let restored_frame = restored.advance(Duration::from_millis(40));
        assert_eq!(restored.state(), original.state());
        assert_eq!(restored_frame, original_frame);
        assert_eq!(restored.state().value, 11);
        assert_eq!(restored.state().steps, vec![Tick(0), Tick(1)]);
    }

    #[test]
    fn snapshot_restore_rejects_incompatible_or_incoherent_state() {
        let runtime = GameRuntime::new(CounterGame::default());

        let mut unsupported = runtime.snapshot();
        unsupported.format_version += 1;
        assert!(GameRuntime::<CounterGame>::from_snapshot(unsupported).is_err());

        let mut incoherent = runtime.snapshot();
        incoherent.completed_tick = Some(Tick(7));
        assert!(GameRuntime::<CounterGame>::from_snapshot(incoherent).is_err());

        let mut invalid_accumulator = runtime.snapshot();
        invalid_accumulator.clock.accumulator_nanos =
            u128::from(invalid_accumulator.clock.step.as_nanos());
        assert!(GameRuntime::<CounterGame>::from_snapshot(invalid_accumulator).is_err());
    }

    #[test]
    fn serialized_replay_reproduces_external_operations_without_internal_duplicates() {
        let config = GameConfig {
            step: StepDuration::from_hz(20),
            max_steps_per_frame: 4,
            max_messages_per_step: 8,
        };
        let mut recorder = GameRecorder::with_config(CounterGame::default(), config);
        recorder.handle_input(HostInputEvent::Trigger(InputTrigger::KeyPressed(
            GameKey::Space,
        )));
        let expected_frames = vec![
            recorder.advance(Duration::from_millis(60)),
            {
                recorder.send(Message::AddAgain);
                recorder.handle_input(HostInputEvent::Trigger(InputTrigger::Tap(
                    SceneNodeId::from_key(&7_u32),
                )));
                recorder.advance(Duration::from_millis(40))
            },
            recorder.advance(Duration::from_millis(50)),
        ];
        let (expected_runtime, replay) = recorder.into_parts();

        let encoded = serde_json::to_string(&replay).unwrap();
        let decoded = serde_json::from_str(&encoded).unwrap();
        let replayed = GameRuntime::<CounterGame>::replay(decoded).unwrap();

        assert_eq!(replayed.runtime.state(), expected_runtime.state());
        assert_eq!(replayed.frames, expected_frames);
        assert_eq!(replayed.runtime.state().value, 14);
        assert_eq!(
            replayed.runtime.state().steps,
            vec![Tick(0), Tick(1), Tick(2)]
        );
        assert_eq!(
            replay.events,
            vec![
                GameReplayEvent::Input(HostInputEvent::Trigger(InputTrigger::KeyPressed(
                    GameKey::Space,
                ))),
                GameReplayEvent::Advance {
                    elapsed_nanos: 60_000_000,
                },
                GameReplayEvent::Message(Message::AddAgain),
                GameReplayEvent::Input(HostInputEvent::Trigger(InputTrigger::Tap(
                    SceneNodeId::from_key(&7_u32),
                ))),
                GameReplayEvent::Advance {
                    elapsed_nanos: 40_000_000,
                },
                GameReplayEvent::Advance {
                    elapsed_nanos: 50_000_000,
                },
            ]
        );
    }

    #[test]
    fn replay_rejects_unsupported_versions_and_out_of_range_time() {
        let recorder = GameRecorder::new(CounterGame::default());
        let (_, mut replay) = recorder.into_parts();
        replay.format_version += 1;
        assert!(GameRuntime::<CounterGame>::replay(replay).is_err());

        let recorder = GameRecorder::new(CounterGame::default());
        let (_, mut replay) = recorder.into_parts();
        replay.events.push(GameReplayEvent::Advance {
            elapsed_nanos: u128::MAX,
        });
        assert!(GameRuntime::<CounterGame>::replay(replay).is_err());
    }
}
