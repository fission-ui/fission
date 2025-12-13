# 7.6 Action Dispatch and Reducer Wiring

This section defines how actions flow from invocation to state mutation, and how reducers are wired into the runtime.
The dispatch model is explicit, deterministic, and fully observable.

Action dispatch is the only mechanism by which state changes occur.

---

## 7.6.1 End-to-End Dispatch Flow

The dispatch pipeline proceeds as follows:

1. An action is invoked (user input, accessibility, test, or programmatic).
2. The target node identity is resolved.
3. The action descriptor and payload are validated.
4. The action is dispatched to the reducer system.
5. State is mutated deterministically.
6. A rebuild is triggered with the new state.

Each step is explicit and instrumentable.

---

## 7.6.2 Action Invocation Sources

Actions may originate from:

- pointer, keyboard, or touch input,
- accessibility APIs,
- test harnesses,
- programmatic dispatch.

All sources converge into the same dispatch path.
There are no special cases.

---

## 7.6.3 Target Resolution

Every action is dispatched with a target `NodeId`.

Rules:
- the target must exist in the current Core snapshot,
- the target must declare the action,
- dispatch to stale or unknown nodes is an error.

Target resolution is deterministic and validated.

---

## 7.6.4 Reducer Registration

Reducers are registered explicitly.

A reducer declares:
- which action types it handles,
- which state type it mutates.

Example (conceptual):

```rust
register_reducer::<Increment>(reduce_increment);
```

Registration is static and deterministic.

---

## 7.6.5 Reducer Execution Model

Reducers are:

- pure functions over state and action,
- executed synchronously,
- isolated from UI structure and rendering.

Reducers:
- may read and write application state,
- must not perform I/O or side effects,
- must not dispatch actions recursively.

Violations are detectable and forbidden.

---

## 7.6.6 Ordering and Determinism

If multiple reducers can handle an action:

- execution order is explicitly defined,
- or ambiguity is rejected at registration time.

Implicit ordering is forbidden.

This ensures reproducible behavior.

---

## 7.6.7 Action Fan-Out and Composition

By default, actions are delivered to a single reducer.

If fan-out is required:
- it must be modeled explicitly,
- reducers must be ordered deterministically,
- state changes must be commutative or ordered.

Implicit broadcast is forbidden.

---

## 7.6.8 Error Handling During Dispatch

Dispatch errors include:

- unknown action tags,
- invalid payloads,
- reducer panics,
- illegal state mutations.

Errors are:
- deterministic,
- surfaced immediately,
- accompanied by provenance and action metadata.

Partial state updates are not permitted.

---

## 7.6.9 Dispatch and Testing

Tests may:

- invoke actions via semantics,
- dispatch actions directly,
- assert intermediate state transitions.

Because dispatch is deterministic:
- tests are repeatable,
- failures are reproducible,
- logs are meaningful.

---

## 7.6.10 Instrumentation and Tracing

Dispatch is fully instrumentable.

Instrumentation can observe:
- action invocation,
- target resolution,
- reducer execution,
- state diffs.

This enables time-travel debugging and replay.

---

## 7.6.11 Relationship to Rebuilds

State changes trigger rebuilds explicitly.

Rules:
- rebuilds occur after reducers complete,
- no partial rebuilds during dispatch,
- rebuild timing is deterministic.

UI updates are a consequence of state change, not action invocation itself.

---

## 7.6.12 Summary

Action dispatch and reducer wiring:

- centralize all state changes,
- eliminate hidden control flow,
- guarantee determinism and observability,
- support testing, replay, and tooling.

They complete the action execution model of the framework.

---
