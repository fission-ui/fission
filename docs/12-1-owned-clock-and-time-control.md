# 12.1 Owned Clock and Time Control

This section defines the **owned clock** model that underpins all animation and time-based behavior.
Time is not ambient or implicit; it is explicit state owned by the runtime and advanced only through actions.

Owning time is a prerequisite for determinism, replay, and reliable testing.

---

## 12.1.1 Motivation

Traditional UI frameworks rely on:
- wall-clock time,
- vsync callbacks,
- frame schedulers.

These introduce:
- nondeterminism,
- platform variance,
- flaky tests,
- irreproducible bugs.

This framework rejects ambient time entirely.

---

## 12.1.2 The Owned Clock Model

The owned clock is a logical clock represented as explicit state.

Properties:
- monotonic,
- integer or fixed-point units,
- advanced only by actions,
- fully observable and serializable.

There is exactly one authoritative clock per runtime instance.

---

## 12.1.3 Time Units and Precision

Time units are fixed and versioned.

Rules:
- units are defined in the Core (e.g. milliseconds in fixed-point),
- precision and rounding are explicit,
- arithmetic overflow behavior is defined.

Floating, platform-dependent time representations are forbidden.

---

## 12.1.4 Advancing Time

Time advances only via actions.

Primary actions include:
- `Tick { dt }`
- `AdvanceTo { time }`

Rules:
- `dt` must be non-negative,
- time never advances implicitly,
- advancing time is deterministic and replayable.

---

## 12.1.5 Relationship to Animation Evaluation

Animation reducers read the owned clock.

Rules:
- animation progress is computed from (current_time - start_time),
- no per-frame callbacks exist,
- skipping time skips animation deterministically.

Animations are pure functions of time and parameters.

---

## 12.1.6 Relationship to Rendering

Rendering does not own time.

Rules:
- renderers never drive time,
- no frame pacing logic exists in the Core,
- rendering consumes snapshots produced at explicit times.

This decouples visual refresh from state evolution.

---

## 12.1.7 Input, Gestures, and Time

Input handling does not implicitly advance time.

Rules:
- gesture streams emit actions,
- time advances only when `Tick` is dispatched,
- input and time are orthogonal streams.

Tests may advance time without input, or vice versa.

---

## 12.1.8 Accessibility and Reduced Motion

Accessibility preferences interact with time deterministically.

Rules:
- reduced-motion may scale or clamp time deltas,
- behavior is explicit and testable,
- time itself is never hidden or skipped.

Reduced motion changes transitions, not semantics.

---

## 12.1.9 Headless Testing and Replay

Owned time enables precise testing.

Examples:

```rust
dispatch(StartAnimation { id: "fade", duration: 300 });
dispatch(Tick { dt: 100 });
dispatch(Tick { dt: 100 });
assert_eq!(find("panel").opacity(), 0.666);
```

The same test produces identical results everywhere.

---

## 12.1.10 Debugging and Inspection

The clock is observable.

Tooling may:
- inspect current time,
- correlate state changes with ticks,
- rewind or fast-forward via replay.

Time is just data.

---

## 12.1.11 Error Handling

Time-related errors include:

- negative deltas,
- time regression,
- overflow beyond representable range.

Such errors are deterministic and fatal by default.

---

## 12.1.12 Summary

Owning the clock ensures that:

- animations are deterministic,
- tests are reliable,
- replay is exact,
- platforms cannot leak timing behavior.

Time is a value the framework owns—not something it reacts to.

---
