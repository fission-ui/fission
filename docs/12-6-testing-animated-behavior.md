# 12.6 Testing Animated Behavior

This section defines how **animated behavior** is tested.
Because animations are explicit, state-driven, and time-controlled, animation tests are deterministic, fast, and fully headless.

Animation tests assert *state evolution*, not visual smoothness.

---

## 12.6.1 Goals of Animation Testing

Animation testing exists to:

- verify correct timing and easing,
- ensure deterministic progression,
- validate interaction with layout and paint,
- detect regressions in animation triggering,
- test accessibility and reduced-motion behavior.

Animation tests must never depend on frame rate or real time.

---

## 12.6.2 Test Harness Model

Animation tests use the standard runtime harness:

- initial state is constructed explicitly,
- animations are started via actions,
- time advances via explicit ticks,
- snapshots are captured and inspected.

There is no special animation test mode.

---

## 12.6.3 Starting Animations in Tests

Animations are started exactly as in production.

Example:

```rust
dispatch(StartAnimation {
    id: "fade",
    target: Opacity("panel"),
    from: 0.0,
    to: 1.0,
    duration: 300,
});
```

Tests verify that animations are registered correctly.

---

## 12.6.4 Advancing Time Deterministically

Time advancement is explicit.

Example:

```rust
dispatch(Tick { dt: 100 });
dispatch(Tick { dt: 100 });
```

Tests may advance time in any pattern without affecting correctness.

---

## 12.6.5 Asserting Intermediate Values

Tests may assert exact intermediate values.

Example:

```rust
assert_eq!(find("panel").opacity(), 0.666);
```

Assertions rely on fixed precision and rounding rules.

---

## 12.6.6 Asserting Completion

Tests may assert completion semantics.

Example:

```rust
dispatch(Tick { dt: 100 });
assert!(animation("fade").is_completed());
assert_eq!(find("panel").opacity(), 1.0);
```

Completion behavior is explicit and testable.

---

## 12.6.7 Testing Cancellation and Replacement

Cancellation and replacement are tested explicitly.

Examples:
- cancelling mid-animation,
- replacing with a new target value,
- verifying sampled start values.

All behaviors are deterministic.

---

## 12.6.8 Testing Layout-Affecting Animations

Animations that affect layout (e.g. size) are tested via snapshots.

Tests may assert:
- layout rect changes over time,
- child positioning under animation,
- absence of layout jitter.

Layout evaluation remains deterministic.

---

## 12.6.9 Testing Paint-Only Animations

Paint-only animations (e.g. opacity, transform) are tested by:

- inspecting resolved state,
- inspecting display lists,
- optionally using golden raster tests.

Visual smoothness is not asserted.

---

## 12.6.10 Reduced Motion Testing

Reduced-motion behavior is tested explicitly.

Example:

```rust
set_config(ReducedMotion::Enabled);
dispatch(StartAnimation { ... });
assert_eq!(find("panel").opacity(), 1.0);
```

Reduced motion affects transitions, not final state.

---

## 12.6.11 Snapshot-Based Regression Tests

Animation regressions are detected via snapshot diffs.

Tests may compare:
- animation registry contents,
- progress values at specific times,
- resolved state fields.

Snapshots explain failures clearly.

---

## 12.6.12 Property-Based and Fuzz Testing

Animations are suitable for property testing.

Examples:
- progress monotonicity,
- bounds preservation,
- idempotence under zero-time ticks.

Fuzzing uses deterministic seeds.

---

## 12.6.13 Common Failure Modes Detected

Animation tests can detect:

- off-by-one timing errors,
- incorrect easing curves,
- nondeterministic progression,
- platform-dependent behavior,
- incorrect reduced-motion handling.

These issues are difficult to catch visually.

---

## 12.6.14 Summary

Testing animated behavior is straightforward because:

- time is explicit,
- animations are pure state machines,
- snapshots expose all relevant data,
- tests assert values, not frames.

If an animation cannot be tested this way, it is not deterministic.

---
