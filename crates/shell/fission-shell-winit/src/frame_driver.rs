use std::sync::Arc;
use std::time::Duration;

#[cfg(not(target_arch = "wasm32"))]
use std::time::Instant;
#[cfg(target_arch = "wasm32")]
use web_time::Instant;

/// Timing supplied to a host-side frame driver before the current frame is
/// built.
///
/// `elapsed` is the same authoritative delta used by Fission's runtime clock.
/// During LiveTest clock advancement it is the requested synthetic duration;
/// otherwise it is elapsed wall time between rendered frames.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FrameDriverContext {
    pub elapsed: Duration,
}

/// Outcome of one host-side frame-driver update.
///
/// The two decisions are intentionally independent: a driver may publish a
/// final state change without scheduling another frame, or may schedule a
/// follow-up frame while its coarse application snapshot remains unchanged.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FrameDriverResult {
    /// Marks the application tree dirty so the updated global state is visible
    /// in the frame currently being built.
    pub state_changed: bool,
    /// Keeps the graphical event loop rendering at its configured frame rate.
    pub request_next_frame: bool,
}

impl FrameDriverResult {
    pub const fn new(state_changed: bool, request_next_frame: bool) -> Self {
        Self {
            state_changed,
            request_next_frame,
        }
    }
}

pub(crate) type FrameDriver<S> =
    Arc<dyn Fn(&mut S, FrameDriverContext) -> FrameDriverResult + Send + Sync + 'static>;

pub(crate) fn invoke<S>(
    driver: Option<&FrameDriver<S>>,
    state: &mut S,
    elapsed: Duration,
) -> FrameDriverResult {
    driver
        .map(|driver| driver(state, FrameDriverContext { elapsed }))
        .unwrap_or_default()
}

/// Returns whether this renderable redraw exists only to build the registry
/// required by a pending startup action.
pub(crate) const fn is_startup_registry_bootstrap(
    startup_dispatched: bool,
    has_startup_action: bool,
) -> bool {
    !startup_dispatched && has_startup_action
}

/// Tracks the wall-clock baseline for frames which are actually eligible to
/// build and render.
///
/// Renderer initialization, zero-sized surfaces, startup registry bootstrap,
/// and platform suspension deliberately do not advance this clock. An
/// explicit LiveTest duration remains pending until the next eligible frame.
#[derive(Debug, Default)]
pub(crate) struct FrameTimingState {
    last_eligible_frame: Option<Instant>,
}

impl FrameTimingState {
    /// Discards the current wall-clock baseline without touching a pending
    /// synthetic clock advance.
    pub(crate) fn rebase(&mut self) {
        self.last_eligible_frame = None;
    }

    /// Resolves the duration for one eligible frame and records `now` as the
    /// next wall-clock baseline.
    ///
    /// A pending synthetic duration is authoritative and is consumed exactly
    /// once. Otherwise the first frame after construction or [`Self::rebase`]
    /// receives zero, paused animation time receives zero, and ordinary frames
    /// receive elapsed wall time rounded down to whole milliseconds to match
    /// the runtime clock.
    pub(crate) fn elapsed_for_eligible_frame(
        &mut self,
        now: Instant,
        animations_paused: bool,
        pending_synthetic_ms: &mut Option<u64>,
    ) -> Duration {
        let elapsed = if let Some(ms) = pending_synthetic_ms.take() {
            Duration::from_millis(ms)
        } else if animations_paused {
            Duration::ZERO
        } else {
            self.last_eligible_frame
                .map(|last| {
                    let elapsed_ms =
                        u64::try_from(now.duration_since(last).as_millis()).unwrap_or(u64::MAX);
                    Duration::from_millis(elapsed_ms)
                })
                .unwrap_or(Duration::ZERO)
        };
        self.last_eligible_frame = Some(now);
        elapsed
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use super::*;

    #[test]
    fn invoke_forwards_exact_elapsed_and_driver_result() {
        let seen = Arc::new(Mutex::new(None));
        let seen_by_driver = seen.clone();
        let driver: FrameDriver<u32> = Arc::new(move |state, context| {
            *state += 1;
            *seen_by_driver.lock().unwrap() = Some(context.elapsed);
            FrameDriverResult::new(true, true)
        });
        let mut state = 8;

        let result = invoke(Some(&driver), &mut state, Duration::from_millis(37));

        assert_eq!(state, 9);
        assert_eq!(*seen.lock().unwrap(), Some(Duration::from_millis(37)));
        assert_eq!(result, FrameDriverResult::new(true, true));
    }

    #[test]
    fn absent_driver_is_idle() {
        let mut state = ();
        assert_eq!(
            invoke::<()>(None, &mut state, Duration::from_secs(1)),
            FrameDriverResult::default()
        );
    }

    #[test]
    fn first_eligible_frame_and_first_frame_after_rebase_have_zero_wall_time() {
        let start = Instant::now();
        let mut timing = FrameTimingState::default();
        let mut synthetic = None;

        assert_eq!(
            timing.elapsed_for_eligible_frame(start, false, &mut synthetic),
            Duration::ZERO
        );
        assert_eq!(
            timing.elapsed_for_eligible_frame(
                start + Duration::from_millis(17),
                false,
                &mut synthetic,
            ),
            Duration::from_millis(17)
        );

        timing.rebase();
        assert_eq!(
            timing.elapsed_for_eligible_frame(
                start + Duration::from_secs(60),
                false,
                &mut synthetic,
            ),
            Duration::ZERO
        );
    }

    #[test]
    fn synthetic_time_survives_rebase_and_is_consumed_once() {
        let start = Instant::now();
        let mut timing = FrameTimingState::default();
        let mut synthetic = Some(37);

        timing.rebase();
        assert_eq!(synthetic, Some(37));
        assert_eq!(
            timing.elapsed_for_eligible_frame(start, true, &mut synthetic),
            Duration::from_millis(37)
        );
        assert_eq!(synthetic, None);
        assert_eq!(
            timing.elapsed_for_eligible_frame(
                start + Duration::from_millis(9),
                true,
                &mut synthetic,
            ),
            Duration::ZERO
        );
    }

    #[test]
    fn only_an_undispatched_startup_action_requires_registry_bootstrap() {
        assert!(is_startup_registry_bootstrap(false, true));
        assert!(!is_startup_registry_bootstrap(false, false));
        assert!(!is_startup_registry_bootstrap(true, true));
        assert!(!is_startup_registry_bootstrap(true, false));
    }
}
