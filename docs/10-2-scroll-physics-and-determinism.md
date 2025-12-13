# 10.2 Scroll Physics and Determinism

This section defines how scroll physics are modeled without compromising determinism.
Physics describe *how scroll state changes over time*, but time itself is owned, explicit, and replayable.

Scroll physics are data-driven and action-based, not implicit or platform-owned.

---

## 10.2.1 Design Goals

Scroll physics must:

- be deterministic and replayable,
- be independent of platform gesture systems,
- operate on explicit time,
- compose with actions and reducers,
- be testable headlessly.

“Natural feel” must never override determinism.

---

## 10.2.2 Explicit Time Ownership

All time used by scroll physics is:

- owned by the runtime,
- advanced explicitly (ticks),
- recorded in action traces.

There is no reliance on wall-clock time, vsync, or timers.

Time advancement itself is an action.

---

## 10.2.3 Physics as Reducer Logic

Scroll physics are implemented in reducers.

Rules:
- physics update scroll offset based on inputs and time,
- reducers are pure functions of state + action,
- no background threads or timers exist.

This makes physics replayable and inspectable.

---

## 10.2.4 Physics Models

Physics models are explicit and configurable.

Examples:
- immediate (no inertia),
- linear deceleration,
- spring-based snapping,
- paged scrolling.

Each model is:
- parameterized,
- versioned,
- deterministic.

No hidden platform defaults exist.

---

## 10.2.5 Input Actions vs Physics Actions

Input actions express intent:

- `ScrollBy { delta }`
- `Fling { velocity }`

Physics actions advance state:

- `Tick { dt }`
- `Settle`

This separation allows:
- clean replay,
- deterministic simulation,
- precise testing.

---

## 10.2.6 Deterministic Integration

Physics integration rules:

- fixed-step integration,
- explicit numeric precision,
- bounded error accumulation,
- clamping via explicit extents.

Variable-step or adaptive integration is forbidden.

---

## 10.2.7 Clamping and Overscroll

Clamping behavior is explicit.

Options include:
- hard clamp,
- elastic overscroll,
- snap-back.

All behaviors are modeled as state transitions, not renderer effects.

---

## 10.2.8 Accessibility Interaction

Accessibility-triggered scroll actions:

- bypass gesture interpretation,
- invoke the same physics paths,
- respect current physics configuration.

There is no “special” accessibility scroll behavior.

---

## 10.2.9 Testing Scroll Physics

Scroll physics are fully testable.

Example:

```rust
dispatch(ScrollBy { delta: 300 });
dispatch(Tick { dt: 16 });
dispatch(Tick { dt: 16 });
assert_eq!(find("list").scroll_offset(), 280);
```

Tests assert exact numeric outcomes.

---

## 10.2.10 Replay and Debugging

Because physics are action-driven:

- full scroll motion can be replayed,
- bugs are reproducible,
- traces can be minimized and analyzed.

This is impossible with platform-driven physics.

---

## 10.2.11 Performance Considerations

Physics computation is lightweight:

- runs in reducers,
- uses simple arithmetic,
- scales linearly with scroll containers.

Parallelism is unnecessary and avoided.

---

## 10.2.12 Versioning and Compatibility

Physics models are versioned.

Rules:
- parameter changes are versioned,
- replay uses the version recorded in the trace,
- incompatible changes require explicit migration.

Physics determinism is preserved across versions.

---

## 10.2.13 Summary

Scroll physics are deterministic because:

- time is explicit,
- physics are reducer logic,
- integration is fixed-step,
- no platform behavior leaks in.

Scrolling “feel” is achieved without sacrificing correctness or testability.
