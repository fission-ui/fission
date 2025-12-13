# 12. Animation System

This section defines the **Animation System** as a deterministic, action-driven mechanism for evolving state over explicit time.
Animations are not callbacks or frame hooks; they are pure state transitions driven by data, ticks, and reducers.

Animation answers one question only: *how state changes over time under explicit control*.

---

## 12.1 Design Goals

The animation system must:

- be deterministic and replayable,
- own time explicitly,
- integrate with the action/reducer model,
- affect layout and painting only via state,
- be testable headlessly without frame loops.

Animations must never depend on wall-clock time or renderer callbacks.

---

## 12.2 Animation as State, Not Behavior

Animations are modeled as **state machines**.

An animation consists of:
- a target state field,
- a start value,
- an end value,
- a duration,
- an easing function,
- a start tick.

There are no closures, callbacks, or implicit lifetimes.

---

## 12.3 Explicit Time Ownership

Time advances only via explicit actions.

Examples:
- `Tick { dt }`
- `AdvanceTo { time }`

Rules:
- no wall-clock time is consulted,
- time units are fixed and versioned,
- identical tick sequences produce identical results.

---

## 12.4 Animation Actions

Animations are created and controlled via actions.

Examples:
- `StartAnimation { id, params }`
- `CancelAnimation { id }`
- `SetAnimationProgress { id, t }`

Actions are recorded, replayed, and inspected like any other.

---

## 12.5 Reducer-Based Evaluation

Reducers evaluate animations on each tick.

Rules:
- reducers are pure functions,
- animation progress is computed deterministically,
- completed animations transition to terminal states.

There is no hidden scheduler.

---

## 12.6 Easing Functions

Easing functions are pure, deterministic functions.

Rules:
- easing functions are versioned,
- numeric precision is explicit,
- custom easings are data-defined, not closures.

Examples include linear, cubic, spring (parameterized).

---

## 12.7 Interaction With Layout and Painting

Animations affect layout and painting only by mutating state.

Examples:
- animating opacity affects paint ops,
- animating size affects layout on next pass,
- animating transform affects painting transforms.

No special animation pipeline exists.

---

## 12.8 Composition and Overlap

Multiple animations may target:

- different fields (composable),
- the same field (resolved explicitly).

Conflict resolution rules:
- last-write-wins,
- additive composition,
- explicit blending.

Rules are explicit and testable.

---

## 12.9 Interruptions and Reversals

Animations can be interrupted deterministically.

Rules:
- interruption is an action,
- current value is sampled explicitly,
- new animation parameters are derived deterministically.

There are no race conditions.

---

## 12.10 Accessibility and Reduced Motion

Accessibility preferences are explicit inputs.

Rules:
- reduced-motion is a configuration flag,
- animations may be disabled or simplified,
- behavior is deterministic and testable.

Reduced motion never changes semantics, only transitions.

---

## 12.11 Headless Testing of Animations

Animations are fully testable headlessly.

Example:

```rust
dispatch(StartAnimation { id: "fade", from: 0.0, to: 1.0, duration: 300 });
dispatch(Tick { dt: 100 });
assert_eq!(find("panel").opacity(), 0.333);
```

Tests assert exact numeric values.

---

## 12.12 Snapshot Representation

Animation state appears in snapshots:

- active animations,
- progress values,
- resolved animated fields.

Snapshots allow diffing and replay of animation behavior.

---

## 12.13 Performance Considerations

Animation evaluation is lightweight:

- simple arithmetic per tick,
- bounded state,
- linear in number of active animations.

Parallelism is unnecessary and avoided.

---

## 12.14 Error Handling

Animation errors include:

- invalid parameters,
- numeric overflow,
- conflicting targets without resolution rules.

Errors are deterministic and surfaced immediately.

---

## 12.15 Summary

The animation system:

- treats time as explicit data,
- evolves state deterministically,
- integrates cleanly with reducers,
- is fully testable and replayable.

Animations are data-driven transitions—not magic.

---
