# 14.4 Event and Action Assertions

This section defines **event and action assertions** used to verify interactive behavior.
Tests assert *what events were produced* and *which actions were dispatched*, not incidental implementation details such as callbacks or timing side effects.

Interaction is validated through explicit data.

---

## 14.4.1 Design Goals

Event and action assertions must:

- be deterministic and replay-safe,
- observe behavior through Core data flows,
- decouple tests from widget internals,
- work uniformly for pointer, keyboard, accessibility, and 3D input,
- explain failures with precise traces.

If an interaction cannot be asserted this way, it is not testable.

---

## 14.4.2 Events vs Actions

The framework distinguishes clearly:

- **Events**: normalized inputs (pointer down, key press, accessibility invoke),
- **Actions**: semantic intents dispatched to reducers.

Tests may assert on either layer, depending on intent.

---

## 14.4.3 Asserting Dispatched Actions

Tests can assert that specific actions were dispatched.

Examples:

```rust
assert_action_dispatched(Increment);
assert_action_dispatched_with(|a: &SetValue| a.value == 3);
```

Rules:
- assertions operate on the action trace,
- action identity is stable and hashable,
- ordering is deterministic.

---

## 14.4.4 Asserting Action Order

Action order is significant and observable.

Example:

```rust
assert_actions([
    Clicked("increment_button"),
    Increment,
    Rebuild,
]);
```

Ordering reflects reducer evaluation order and is snapshot-visible.

---

## 14.4.5 Negative Assertions

Tests may assert that actions did *not* occur.

Example:

```rust
assert_no_action(Submit);
```

Negative assertions are evaluated over bounded traces.

---

## 14.4.6 Event Injection in Tests

Tests inject events explicitly.

Examples:

```rust
click("increment_button");
key_press(Key::Enter);
accessibility_activate("submit");
```

Event injection produces normalized events before actions.

---

## 14.4.7 Event-to-Action Mapping Assertions

Tests may assert the mapping from events to actions.

Example:

```rust
click("increment_button");
assert_action_dispatched(Increment);
```

This verifies wiring without inspecting widget code.

---

## 14.4.8 Accessibility Action Assertions

Accessibility interactions are first-class.

Examples:

```rust
accessibility_activate("play_button");
assert_action_dispatched(Play);
```

Accessibility and pointer interactions must be behaviorally equivalent.

---

## 14.4.9 3D Interaction Assertions

3D interactions surface as actions.

Examples:

```rust
pick("object_42");
assert_action_dispatched(ObjectSelected("object_42"));
```

Tests assert intent, not engine-specific behavior.

---

## 14.4.10 Action Payload Inspection

Assertions may inspect action payloads.

Rules:
- payloads are immutable,
- comparison is structural,
- floating values follow pinned precision rules.

Payload inspection enables fine-grained validation.

---

## 14.4.11 Action Traces and Windows

Assertions operate over trace windows.

Rules:
- tests may assert over the last N actions,
- traces are segmented by frame or tick,
- trace size limits are explicit.

This prevents unbounded history dependence.

---

## 14.4.12 Error Conditions

Action assertion errors include:

- missing expected actions,
- unexpected extra actions,
- incorrect ordering,
- payload mismatches.

Errors include structured diffs and traces.

---

## 14.4.13 Determinism Guarantees

Event and action assertions are deterministic because:

- events are normalized,
- actions are explicit data,
- reducers are pure,
- time is controlled.

Identical tests always produce identical traces.

---

## 14.4.14 Summary

Event and action assertions work because:

- interaction is modeled as data,
- intent is explicit and observable,
- accessibility and 3D share the same path,
- failures are explainable through traces.

If a UI interaction cannot be asserted via actions, it is not correctly modeled.

---
