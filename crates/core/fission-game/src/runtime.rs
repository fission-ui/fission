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
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GameConfig {
    pub step: StepDuration,
    pub max_steps_per_frame: u32,
    pub max_messages_per_step: u32,
}

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

    #[derive(Clone, Debug, Default, PartialEq, Eq)]
    struct CounterGame {
        value: i32,
        steps: Vec<Tick>,
    }

    impl GameState for CounterGame {}

    #[derive(Clone, Debug, PartialEq, Eq)]
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
}
