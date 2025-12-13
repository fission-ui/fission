# 10.3 Input Handling and Offset Updates

This section defines how user input is translated into scroll state updates.
Input handling is declarative and action-based: input never mutates scroll state directly.

Scrolling behavior emerges from explicit actions processed by reducers.

---

## 10.3.1 Design Principles

Input handling for scrolling must:

- be deterministic and replayable,
- separate intent from effect,
- unify mouse, touch, keyboard, and accessibility input,
- avoid platform-specific behavior,
- integrate cleanly with scroll physics.

Input is interpreted, not executed.

---

## 10.3.2 Input Normalization Layer

Raw platform input is normalized into framework input events.

Examples:
- mouse wheel deltas,
- touch drag vectors,
- keyboard navigation commands,
- accessibility scroll requests.

Normalization:
- removes platform variance,
- converts input into logical units,
- produces deterministic events.

---

## 10.3.3 Hit Testing for Scroll Targets

Scroll input is routed via hit testing.

Rules:
- hit testing uses layout snapshot + PaintMap,
- the nearest scroll container in the ancestry is selected,
- hit testing respects clip bounds.

Scroll routing is deterministic and observable.

---

## 10.3.4 From Input Events to Actions

Normalized input events are converted into actions.

Examples:
- wheel → `ScrollBy { delta }`
- drag → `ScrollBy { delta }`
- key → `ScrollBy { delta }` or `PageDown`
- accessibility → `ScrollTo { position }`

This conversion is explicit and configurable.

---

## 10.3.5 Action Dispatch

Scroll actions are dispatched through the standard action system.

Rules:
- actions are timestamped or tick-relative,
- actions are recorded in traces,
- dispatch order is deterministic.

No special scroll dispatch path exists.

---

## 10.3.6 Reducer Processing

Reducers process scroll actions by:

- validating target scroll container,
- applying physics rules,
- computing new scroll offsets,
- enforcing extents and clamping.

Reducers are pure and side-effect free.

---

## 10.3.7 Offset Update Semantics

Scroll offset updates follow strict rules:

- offsets are updated only in reducers,
- updates are atomic per action,
- partial updates are forbidden.

All offset changes are observable state transitions.

---

## 10.3.8 Gesture State Tracking

Gesture-related state (e.g. drag in progress) is modeled explicitly.

Rules:
- gesture state lives in framework state,
- transitions are driven by actions,
- no hidden platform gesture state exists.

This ensures full replayability.

---

## 10.3.9 Continuous Input and Ticking

Continuous input (e.g. touch drag) is handled by:

- emitting a sequence of actions,
- advancing time explicitly via `Tick` actions,
- applying physics deterministically.

Input rate does not affect correctness.

---

## 10.3.10 Accessibility Input Path

Accessibility input uses the same path:

- hit testing identifies scroll container,
- actions are dispatched,
- reducers update scroll state.

There is no separate accessibility scroll implementation.

---

## 10.3.11 Testing Input and Scrolling

Input-driven scrolling is fully testable.

Example:

```rust
send_input(MouseWheel { delta: -120 });
dispatch(Tick { dt: 16 });
assert_eq!(find("list").scroll_offset(), 120);
```

Tests assert state, not gestures.

---

## 10.3.12 Error Handling

Input handling errors include:

- ambiguous scroll targets,
- invalid scroll containers,
- out-of-range offsets.

Errors are deterministic and reported with provenance.

---

## 10.3.13 Summary

Input handling and offset updates:

- translate intent into explicit actions,
- route deterministically via hit testing,
- update scroll state only via reducers,
- unify all input sources.

Scrolling is controlled, explicit, and fully testable.

---
