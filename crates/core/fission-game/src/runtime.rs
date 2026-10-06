//! Deterministic fixed-step simulation, snapshot, replay, and headless testing.

use std::collections::VecDeque;
use std::fmt::Debug;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::{
    DeterministicRandom, FixedStepClock, FixedStepClockSnapshot, HostInputEvent, InputMap,
    InputState, RandomSnapshot, StepBatch, StepDuration, Tick,
};

pub trait GameState: Clone + 'static {}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GameConfig {
    pub step: StepDuration,
    pub max_steps_per_frame: u32,
    pub max_messages_per_step: u32,
    pub random_seed: u64,
}

impl Default for GameConfig {
    fn default() -> Self {
        Self {
            step: StepDuration::from_hz(60),
            max_steps_per_frame: 8,
            max_messages_per_step: 1_024,
            random_seed: 0xF155_10A5_D37E_4D1C,
        }
    }
}

impl GameConfig {
    fn validated(self) -> Self {
        assert!(self.max_steps_per_frame > 0);
        assert!(self.max_messages_per_step > 0);
        self
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct GameTime {
    pub completed_tick: Option<Tick>,
    pub interpolation: f32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum RuntimeDiagnostic {
    DroppedSimulationTime { nanos: u128 },
    MessageBudgetExceeded { tick: Tick, deferred: usize },
}

pub struct GameCtx<'a, G: Game> {
    tick: Tick,
    input: &'a InputState,
    random: &'a mut DeterministicRandom,
    pending: &'a mut VecDeque<G::Message>,
}

impl<G: Game> GameCtx<'_, G> {
    pub const fn tick(&self) -> Tick {
        self.tick
    }

    pub const fn input(&self) -> &InputState {
        self.input
    }

    pub fn random(&mut self) -> &mut DeterministicRandom {
        self.random
    }

    pub fn send(&mut self, message: G::Message) {
        self.pending.push_back(message);
    }
}

pub struct StepCtx<'a, G: Game> {
    tick: Tick,
    duration: StepDuration,
    input: &'a InputState,
    random: &'a mut DeterministicRandom,
    pending: &'a mut VecDeque<G::Message>,
}

impl<G: Game> StepCtx<'_, G> {
    pub const fn tick(&self) -> Tick {
        self.tick
    }

    pub const fn duration(&self) -> StepDuration {
        self.duration
    }

    pub const fn input(&self) -> &InputState {
        self.input
    }

    pub fn random(&mut self) -> &mut DeterministicRandom {
        self.random
    }

    pub fn send(&mut self, message: G::Message) {
        self.pending.push_back(message);
    }
}

/// One authoritative simulation with a renderer-neutral presentation value.
pub trait Game: GameState + Sized {
    type Message: Clone + Debug + 'static;
    type Presentation: Clone + Debug + PartialEq + 'static;

    fn input(_input: &mut InputMap<Self::Message>) {}
    fn react(&mut self, _message: Self::Message, _ctx: &mut GameCtx<'_, Self>) {}
    fn step(&mut self, ctx: &mut StepCtx<'_, Self>);
    fn present(&self, time: GameTime) -> Self::Presentation;
}

#[derive(Clone, Debug, PartialEq)]
pub struct GameFrame<P> {
    pub steps: StepBatch,
    pub time: GameTime,
    pub presentation: P,
    pub diagnostics: Vec<RuntimeDiagnostic>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GameSnapshot<S, M> {
    pub format_version: u16,
    pub game: S,
    pub pending_messages: Vec<M>,
    pub input: InputState,
    pub random: RandomSnapshot,
    pub clock: FixedStepClockSnapshot,
    pub config: GameConfig,
    pub completed_tick: Option<Tick>,
}

impl<S, M> GameSnapshot<S, M> {
    pub const FORMAT_VERSION: u16 = 1;
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GameSnapshotError(String);

impl std::fmt::Display for GameSnapshotError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for GameSnapshotError {}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "operation")]
pub enum GameReplayEvent<M> {
    Input(HostInputEvent),
    Message(M),
    Advance { elapsed_nanos: u128 },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GameReplay<S, M> {
    pub format_version: u16,
    pub initial_snapshot: GameSnapshot<S, M>,
    pub events: Vec<GameReplayEvent<M>>,
}

impl<S, M> GameReplay<S, M> {
    pub const FORMAT_VERSION: u16 = 1;
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GameReplayError(String);

impl std::fmt::Display for GameReplayError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for GameReplayError {}

pub struct GameRuntime<G: Game> {
    game: G,
    input_map: InputMap<G::Message>,
    input: InputState,
    pending: VecDeque<G::Message>,
    random: DeterministicRandom,
    clock: FixedStepClock,
    config: GameConfig,
    completed_tick: Option<Tick>,
}

pub struct GameReplayRun<G: Game> {
    pub runtime: GameRuntime<G>,
    pub frames: Vec<GameFrame<G::Presentation>>,
}

impl<G: Game> GameRuntime<G> {
    pub fn new(game: G) -> Self {
        Self::with_config(game, GameConfig::default())
    }

    pub fn with_config(game: G, config: GameConfig) -> Self {
        let config = config.validated();
        let mut input_map = InputMap::new();
        G::input(&mut input_map);
        Self {
            game,
            input_map,
            input: InputState::default(),
            pending: VecDeque::new(),
            random: DeterministicRandom::seeded(config.random_seed),
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

    pub const fn input_state(&self) -> &InputState {
        &self.input
    }

    pub fn handle_input(&mut self, event: HostInputEvent) {
        if let Some(trigger) = event.trigger() {
            self.pending.extend(self.input_map.messages_for(&trigger));
        }
        self.input.apply(&event);
    }

    pub fn send(&mut self, message: G::Message) {
        self.pending.push_back(message);
    }

    pub fn snapshot(&self) -> GameSnapshot<G, G::Message> {
        GameSnapshot {
            format_version: GameSnapshot::<G, G::Message>::FORMAT_VERSION,
            game: self.game.clone(),
            pending_messages: self.pending.iter().cloned().collect(),
            input: self.input.clone(),
            random: self.random.snapshot(),
            clock: self.clock.snapshot(),
            config: self.config,
            completed_tick: self.completed_tick,
        }
    }

    pub fn from_snapshot(snapshot: GameSnapshot<G, G::Message>) -> Result<Self, GameSnapshotError> {
        if snapshot.format_version != GameSnapshot::<G, G::Message>::FORMAT_VERSION {
            return Err(GameSnapshotError(
                "unsupported game snapshot version".into(),
            ));
        }
        if snapshot.config.step != snapshot.clock.step
            || snapshot.config.max_steps_per_frame != snapshot.clock.max_steps_per_frame
        {
            return Err(GameSnapshotError(
                "snapshot clock and configuration disagree".into(),
            ));
        }
        let expected_next_tick = snapshot
            .completed_tick
            .map_or(0, |tick| tick.0.saturating_add(1));
        if snapshot.clock.next_tick.0 != expected_next_tick {
            return Err(GameSnapshotError("snapshot tick and clock disagree".into()));
        }
        let clock = FixedStepClock::from_snapshot(snapshot.clock)
            .ok_or_else(|| GameSnapshotError("snapshot clock is invalid".into()))?;
        let config = snapshot.config.validated();
        let mut input_map = InputMap::new();
        G::input(&mut input_map);
        Ok(Self {
            game: snapshot.game,
            input_map,
            input: snapshot.input,
            pending: snapshot.pending_messages.into_iter().collect(),
            random: DeterministicRandom::from_snapshot(snapshot.random),
            clock,
            config,
            completed_tick: snapshot.completed_tick,
        })
    }

    pub fn replay(replay: GameReplay<G, G::Message>) -> Result<GameReplayRun<G>, GameReplayError> {
        if replay.format_version != GameReplay::<G, G::Message>::FORMAT_VERSION {
            return Err(GameReplayError("unsupported game replay version".into()));
        }
        let mut runtime = Self::from_snapshot(replay.initial_snapshot)
            .map_err(|error| GameReplayError(format!("invalid replay snapshot: {error}")))?;
        let mut frames = Vec::new();
        for (index, event) in replay.events.into_iter().enumerate() {
            match event {
                GameReplayEvent::Input(event) => runtime.handle_input(event),
                GameReplayEvent::Message(message) => runtime.send(message),
                GameReplayEvent::Advance { elapsed_nanos } => {
                    let elapsed = duration_from_nanos(elapsed_nanos).ok_or_else(|| {
                        GameReplayError(format!("replay event {index} duration is out of range"))
                    })?;
                    frames.push(runtime.advance(elapsed));
                }
            }
        }
        Ok(GameReplayRun { runtime, frames })
    }

    pub fn advance(&mut self, elapsed: Duration) -> GameFrame<G::Presentation> {
        let steps = self.clock.advance(elapsed);
        let mut diagnostics = Vec::new();
        if !steps.dropped.is_zero() {
            diagnostics.push(RuntimeDiagnostic::DroppedSimulationTime {
                nanos: steps.dropped.as_nanos(),
            });
        }
        for tick in steps.ticks() {
            self.deliver_messages(tick, &mut diagnostics);
            let mut ctx = StepCtx::<G> {
                tick,
                duration: self.config.step,
                input: &self.input,
                random: &mut self.random,
                pending: &mut self.pending,
            };
            self.game.step(&mut ctx);
            self.completed_tick = Some(tick);
        }
        let time = GameTime {
            completed_tick: self.completed_tick,
            interpolation: steps.interpolation,
        };
        GameFrame {
            steps,
            time,
            presentation: self.game.present(time),
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
                input: &self.input,
                random: &mut self.random,
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

pub struct GameRecorder<G: Game> {
    runtime: GameRuntime<G>,
    replay: GameReplay<G, G::Message>,
}

impl<G: Game> GameRecorder<G> {
    pub fn new(game: G) -> Self {
        Self::from_runtime(GameRuntime::new(game))
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

    pub fn advance(&mut self, elapsed: Duration) -> GameFrame<G::Presentation> {
        self.replay.events.push(GameReplayEvent::Advance {
            elapsed_nanos: elapsed.as_nanos(),
        });
        self.runtime.advance(elapsed)
    }

    pub const fn state(&self) -> &G {
        self.runtime.state()
    }

    pub fn finish(self) -> GameReplay<G, G::Message> {
        self.replay
    }
}

pub struct GameTestHarness<G: Game> {
    runtime: GameRuntime<G>,
}

impl<G: Game> GameTestHarness<G> {
    pub fn new(game: G) -> Self {
        Self {
            runtime: GameRuntime::new(game),
        }
    }

    pub fn input(&mut self, event: HostInputEvent) {
        self.runtime.handle_input(event);
    }

    pub fn send(&mut self, message: G::Message) {
        self.runtime.send(message);
    }

    pub fn tick(&mut self) -> GameFrame<G::Presentation> {
        self.runtime.advance(self.runtime.config.step.as_duration())
    }

    pub fn ticks(&mut self, count: u32) -> Vec<GameFrame<G::Presentation>> {
        (0..count).map(|_| self.tick()).collect()
    }

    pub const fn state(&self) -> &G {
        self.runtime.state()
    }
}

fn duration_from_nanos(nanos: u128) -> Option<Duration> {
    let seconds = u64::try_from(nanos / 1_000_000_000).ok()?;
    let subsecond = (nanos % 1_000_000_000) as u32;
    Some(Duration::new(seconds, subsecond))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{GameKey, InputTrigger};

    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    struct Counter {
        value: u32,
        random: Vec<u64>,
    }

    impl GameState for Counter {}

    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    enum Message {
        Add,
    }

    impl Game for Counter {
        type Message = Message;
        type Presentation = u32;

        fn input(input: &mut InputMap<Self::Message>) {
            input
                .on(InputTrigger::KeyPressed {
                    key: GameKey::Space,
                })
                .send(Message::Add);
        }

        fn react(&mut self, message: Self::Message, _ctx: &mut GameCtx<'_, Self>) {
            match message {
                Message::Add => self.value += 1,
            }
        }

        fn step(&mut self, ctx: &mut StepCtx<'_, Self>) {
            self.random.push(ctx.random().next_u64());
        }

        fn present(&self, _time: GameTime) -> Self::Presentation {
            self.value
        }
    }

    fn counter() -> Counter {
        Counter {
            value: 0,
            random: vec![],
        }
    }

    #[test]
    fn input_is_applied_only_on_the_next_fixed_tick() {
        let mut runtime = GameRuntime::new(counter());
        runtime.handle_input(HostInputEvent::Key {
            key: GameKey::Space,
            pressed: true,
        });
        assert_eq!(runtime.advance(Duration::ZERO).presentation, 0);
        assert_eq!(
            runtime
                .advance(StepDuration::from_hz(60).as_duration())
                .presentation,
            1
        );
    }

    #[test]
    fn snapshot_restore_preserves_tick_input_queue_and_random_stream() {
        let mut runtime = GameRuntime::new(counter());
        runtime.send(Message::Add);
        runtime.advance(StepDuration::from_hz(60).as_duration());
        runtime.send(Message::Add);
        let snapshot = runtime.snapshot();

        let expected = runtime.advance(StepDuration::from_hz(60).as_duration());
        let mut restored = GameRuntime::from_snapshot(snapshot).unwrap();
        let actual = restored.advance(StepDuration::from_hz(60).as_duration());

        assert_eq!(actual, expected);
        assert_eq!(restored.state(), runtime.state());
    }

    #[test]
    fn recorded_run_replays_to_the_same_state_and_frames() {
        let mut recorder = GameRecorder::new(counter());
        recorder.handle_input(HostInputEvent::Key {
            key: GameKey::Space,
            pressed: true,
        });
        let expected = recorder.advance(StepDuration::from_hz(60).as_duration());
        let replay = recorder.finish();

        let run = GameRuntime::<Counter>::replay(replay).unwrap();
        assert_eq!(run.frames, vec![expected]);
        assert_eq!(run.runtime.state().value, 1);
    }
}
