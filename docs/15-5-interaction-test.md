# 15.5 Interaction Test

This section presents a concrete **interaction test** for the Counter application.
The test verifies that user interaction results in the correct action dispatch, state mutation, layout stability, and observable snapshot changes—without rendering or timing ambiguity.

The test exercises the full stack under deterministic control.

---

## 15.5.1 Test Intent

The interaction test must verify that:

- activating the button dispatches the correct action,
- the reducer updates state exactly once per activation,
- the UI snapshot reflects the updated state,
- no unintended actions are emitted,
- layout and geometry remain stable.

This test asserts *behavior*, not implementation details.

---

## 15.5.2 Test Harness Setup

```rust
let mut harness = TestHarness::new(CounterApp::default());
```

Configuration assumptions:
- deterministic clock enabled,
- headless renderer selected,
- mock backends registered,
- instrumentation enabled for tracing.

---

## 15.5.3 Initial Assertions

Before interaction, assert initial state and structure.

```rust
assert_eq!(find("counter_text").text(), "Count: 0");
assert_eq!(find("increment_button").rect().width, 88);
assert_no_action(CounterAction::Increment);
```

These establish a known baseline.

---

## 15.5.4 Injecting the Interaction

Simulate a user activating the button.

```rust
click("increment_button");
```

Notes:
- `click` injects a normalized pointer event,
- hit testing selects the button via layout snapshot,
- semantics map the event to an action.

No callbacks are invoked.

---

## 15.5.5 Asserting Action Dispatch

Verify that the correct action was dispatched.

```rust
assert_action_dispatched(CounterAction::Increment);
```

Optional ordering assertion:

```rust
assert_actions_in_order([
    CounterAction::Increment,
]);
```

Action identity and ordering are deterministic.

---

## 15.5.6 Asserting State and Snapshot Update

After the interaction, assert state-derived UI updates.

```rust
assert_eq!(find("counter_text").text(), "Count: 1");
```

The snapshot reflects the reducer’s effect.

---

## 15.5.7 Ensuring No Extra Actions

Verify that no unintended actions occurred.

```rust
assert_action_count::<CounterAction>(1);
```

This guards against duplicate dispatches or hidden side effects.

---

## 15.5.8 Accessibility Equivalence Test

Repeat the interaction via accessibility activation.

```rust
accessibility_activate("increment_button");
assert_eq!(find("counter_text").text(), "Count: 2");
```

Accessibility and pointer input share the same action path.

---

## 15.5.9 Geometry Stability Assertion

Ensure layout did not change unexpectedly.

```rust
let rect_before = find("increment_button").rect();
click("increment_button");
let rect_after = find("increment_button").rect();
assert_eq!(rect_before, rect_after);
```

State changes do not cause unintended layout churn.

---

## 15.5.10 Structural Snapshot Diff (Optional)

Optionally assert structural stability.

```rust
assert_no_structural_diff();
```

Only content changed; structure did not.

---

## 15.5.11 Failure Diagnostics

If this test fails, diagnostics include:

- action trace showing dispatched actions,
- before/after snapshots,
- structural and geometry diffs.

Failures are immediately explainable.

---

## 15.5.12 Why This Test Is Reliable

This interaction test is reliable because:

- time is not involved,
- rendering is not required,
- input is normalized,
- behavior is modeled as data.

It will not flake across machines or platforms.

---

## 15.5.13 Summary

The Counter interaction test demonstrates that:

- UI behavior is testable end-to-end,
- actions are the unit of interaction,
- snapshots are the unit of observation,
- determinism eliminates flakiness.

This is the standard pattern for interaction testing in the framework.

---
