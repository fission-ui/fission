//! Deterministic game-runtime primitives shared by 2D and 3D Fission games.
//!
//! This foundation has no renderer, window, network, or product dependency and
//! can therefore be used by headless tests and every graphical Fission shell.

mod collision;
mod geometry;
mod identity;
mod runtime;
mod scene2d;

pub use collision::{Area2D, TouchArea, Touchable2D};
pub use geometry::{Bounds2D, Degrees, Place, Px, PxPerSecond, Size};
pub use identity::{StableKey, StableKeyValue, StableSymbol};
pub use runtime::{
    Game, GameConfig, GameCtx, GameFrame, GameKey, GameRuntime, GameState, GameTestHarness,
    GameTime, GameView, HostInputEvent, InputBinding, InputMap, InputTrigger, RuntimeDiagnostic,
    StepCtx,
};
pub use scene2d::{
    Anchor, BlendMode2D, GameDiagnostic, ImageAsset, ImageInstance2D, ImageSampling, Layer,
    Scene2D, Scene2DCommand, Scene2DIR, SceneNodeId, Transform2D,
};

use std::time::Duration;

use serde::{Deserialize, Serialize};

/// Monotonic fixed simulation-step number.
#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
#[serde(transparent)]
pub struct Tick(pub u64);

/// Fixed duration of one simulation step.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StepDuration {
    nanos: u64,
}

impl StepDuration {
    /// Creates a non-zero fixed step from a frequency such as 20 or 60 Hz.
    pub fn from_hz(hz: u32) -> Self {
        assert!(hz > 0, "simulation frequency must be non-zero");
        Self {
            nanos: 1_000_000_000 / u64::from(hz),
        }
    }

    pub const fn from_nanos(nanos: u64) -> Self {
        assert!(nanos > 0, "simulation step must be non-zero");
        Self { nanos }
    }

    pub const fn as_nanos(self) -> u64 {
        self.nanos
    }

    pub fn as_duration(self) -> Duration {
        Duration::from_nanos(self.nanos)
    }

    pub fn as_secs_f32(self) -> f32 {
        self.nanos as f32 / 1_000_000_000.0
    }
}

/// Result of adding presentation time to a [`FixedStepClock`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StepBatch {
    /// First tick to execute. The batch covers `first_tick..=last_tick()`.
    pub first_tick: Tick,
    /// Number of fixed simulation steps to execute now.
    pub steps: u32,
    /// Presentation interpolation from the latest completed state to the next.
    pub interpolation: f32,
    /// Time deliberately discarded to prevent an unbounded catch-up spiral.
    pub dropped: Duration,
}

impl StepBatch {
    pub fn last_tick(self) -> Option<Tick> {
        self.steps
            .checked_sub(1)
            .map(|offset| Tick(self.first_tick.0 + u64::from(offset)))
    }

    pub fn ticks(self) -> impl Iterator<Item = Tick> {
        (0..self.steps).map(move |offset| Tick(self.first_tick.0 + u64::from(offset)))
    }
}

/// Fixed-step scheduler which separates presentation time from simulation time.
#[derive(Clone, Debug)]
pub struct FixedStepClock {
    step: StepDuration,
    max_steps_per_frame: u32,
    next_tick: Tick,
    accumulator: Duration,
}

impl FixedStepClock {
    pub fn new(step: StepDuration) -> Self {
        Self {
            step,
            max_steps_per_frame: 8,
            next_tick: Tick(0),
            accumulator: Duration::ZERO,
        }
    }

    /// Bounds work performed after a stall. Excess complete steps are reported
    /// as dropped rather than making one render frame run indefinitely.
    pub fn with_max_steps_per_frame(mut self, maximum: u32) -> Self {
        assert!(maximum > 0, "maximum steps per frame must be non-zero");
        self.max_steps_per_frame = maximum;
        self
    }

    pub const fn step(&self) -> StepDuration {
        self.step
    }

    pub const fn next_tick(&self) -> Tick {
        self.next_tick
    }

    pub fn advance(&mut self, elapsed: Duration) -> StepBatch {
        self.accumulator = self.accumulator.saturating_add(elapsed);
        let step = self.step.as_duration();
        let available =
            (self.accumulator.as_nanos() / step.as_nanos()).min(u128::from(u32::MAX)) as u32;
        let steps = available.min(self.max_steps_per_frame);
        let discarded_steps = available.saturating_sub(steps);
        let consumed = step.saturating_mul(steps);
        let dropped = step.saturating_mul(discarded_steps);
        self.accumulator = self
            .accumulator
            .saturating_sub(consumed)
            .saturating_sub(dropped);

        let first_tick = self.next_tick;
        self.next_tick.0 = self.next_tick.0.saturating_add(u64::from(steps));
        let interpolation = (self.accumulator.as_secs_f64() / step.as_secs_f64()) as f32;
        StepBatch {
            first_tick,
            steps,
            interpolation,
            dropped,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn equal_elapsed_time_produces_equal_tick_sequences() {
        let mut first = FixedStepClock::new(StepDuration::from_hz(20));
        let mut second = first.clone();
        let a: Vec<_> = [10, 40, 75, 25]
            .into_iter()
            .flat_map(|ms| first.advance(Duration::from_millis(ms)).ticks())
            .collect();
        let b: Vec<_> = [50, 50, 50]
            .into_iter()
            .flat_map(|ms| second.advance(Duration::from_millis(ms)).ticks())
            .collect();
        assert_eq!(a, b);
        assert_eq!(a, vec![Tick(0), Tick(1), Tick(2)]);
    }

    #[test]
    fn catch_up_is_bounded_and_excess_time_is_reported() {
        let mut clock = FixedStepClock::new(StepDuration::from_hz(10)).with_max_steps_per_frame(2);
        let batch = clock.advance(Duration::from_millis(550));
        assert_eq!(batch.steps, 2);
        assert_eq!(batch.dropped, Duration::from_millis(300));
        assert!((batch.interpolation - 0.5).abs() < 0.001);
    }
}
