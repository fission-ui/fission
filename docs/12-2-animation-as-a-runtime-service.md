# 12.2 Animation as a Runtime Service

This section defines animation as a **runtime service** rather than a widget concern or rendering feature.
Animations live alongside layout, input, and media as a first-class system owned by the Core runtime.

Animations orchestrate state over time; they do not belong to the widget tree.

---

## 12.2.1 Motivation

Embedding animation logic inside widgets or renderers leads to:

- duplicated scheduling logic,
- hidden lifetimes,
- nondeterministic timing,
- poor testability.

By centralizing animation in the runtime, the system gains:
- determinism,
- uniform behavior,
- global observability,
- simpler authoring semantics.

---

## 12.2.2 Service-Oriented Model

The animation system is modeled as a runtime service:

- it owns animation instances,
- it evaluates progress based on the owned clock,
- it mutates application state via reducers,
- it exposes inspection and snapshot data.

Widgets merely *declare intent* to animate.

---

## 12.2.3 Animation Registry

The runtime maintains an animation registry.

Each entry includes:
- animation identifier,
- target state path,
- start and end values,
- timing parameters,
- easing function,
- lifecycle state.

The registry is deterministic and serializable.

---

## 12.2.4 Lifecycle of an Animation

Animations follow an explicit lifecycle:

1. Requested (via action)
2. Registered
3. Active
4. Completed or Cancelled
5. Removed

Each transition is explicit and observable.

---

## 12.2.5 Interaction With Reducers

Reducers interact with the animation service by:

- registering new animations,
- cancelling or replacing existing ones,
- reading resolved animated values.

Reducers remain pure; the service manages orchestration.

---

## 12.2.6 Separation From Widgets

Widgets do not:
- store animation timers,
- compute interpolation,
- react to frame callbacks.

Instead, widgets bind visual or layout properties to state fields that may be animated.

This keeps widgets declarative and simple.

---

## 12.2.7 Interaction With Layout and Painting

The animation service does not perform layout or painting.

Rules:
- it mutates state only,
- layout reacts to state changes,
- painting reacts to layout and state.

This preserves pipeline clarity and determinism.

---

## 12.2.8 Composition and Coordination

As a centralized service, animations can:

- coordinate across multiple nodes,
- synchronize multiple properties,
- enforce global rules (e.g. reduced motion).

Cross-widget coordination is straightforward.

---

## 12.2.9 Cancellation and Replacement Semantics

Animation cancellation is explicit.

Rules:
- cancelling an animation is an action,
- replacement policies are explicit,
- intermediate values are sampled deterministically.

There are no implicit drops or leaks.

---

## 12.2.10 Snapshot and Inspection Support

The animation service exposes snapshot data:

- active animations,
- progress values,
- resolved state contributions.

Tooling can inspect animation state at any tick.

---

## 12.2.11 Headless Testing

Because animation is a runtime service:

- no rendering loop is required,
- time can be advanced explicitly,
- state changes are asserted directly.

Animations are trivial to test in isolation.

---

## 12.2.12 Performance Considerations

Centralized animation evaluation is efficient:

- O(n) over active animations,
- simple arithmetic per tick,
- cache-friendly state layout.

Scalability is predictable and controllable.

---

## 12.2.13 Error Handling

Animation service errors include:

- invalid target paths,
- conflicting animation ownership,
- invalid timing parameters.

Errors are detected eagerly and reported deterministically.

---

## 12.2.14 Summary

Treating animation as a runtime service:

- decouples animation from widgets and rendering,
- guarantees deterministic behavior,
- simplifies testing and tooling,
- enables coordinated, global animation policies.

Animations are part of the runtime—not UI sugar.

---
