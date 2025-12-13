# 12.4 Deterministic Frame Advancement

This section defines how the framework advances *frames* deterministically.
A “frame” is not a render callback; it is a logical evaluation step driven by explicit time advancement and pipeline execution.

Frame advancement is a consequence of state changes, not a driver of them.

---

## 12.4.1 Reframing the Concept of a Frame

In this framework, a frame is:

- a snapshot of state at a specific logical time,
- the result of running reducers, layout, and paint,
- fully reproducible from inputs and time.

There is no implicit frame loop.

---

## 12.4.2 What Advances a Frame

A new frame is produced when:

- time advances (`Tick`, `AdvanceTo`),
- state changes due to actions,
- layout or paint is invalidated.

If nothing changes, no new frame exists.

---

## 12.4.3 Frame Advancement Is Explicit

Rules:
- the runtime never schedules frames on its own,
- platforms may *request* a frame, not force one,
- frame production is deterministic given inputs.

Frame advancement is observable and testable.

---

## 12.4.4 Relationship Between Time and Frames

Time and frames are related but distinct.

- Time advancement may produce multiple frames.
- Multiple state changes at the same time may collapse into one frame.
- Frames always correspond to a specific clock value.

There is no concept of “missed frames.”

---

## 12.4.5 Fixed-Step vs Variable-Step

The framework supports fixed-step advancement by convention.

Rules:
- `Tick { dt }` defines the step size,
- reducers assume fixed-step semantics,
- variable-step logic is forbidden in Core systems.

If variable rates are needed, they must be quantized explicitly.

---

## 12.4.6 Animation Evaluation and Frames

Animation evaluation occurs during frame advancement.

Rules:
- animation progress is recomputed for each frame,
- progress depends only on owned time,
- no interpolation is done during rendering.

Frames contain fully resolved animation values.

---

## 12.4.7 Rendering and Frame Consumption

Renderers consume completed frames.

Rules:
- renderers never mutate state,
- renderers never advance time,
- renderers may drop or skip frames without semantic impact.

Dropped frames do not affect correctness.

---

## 12.4.8 Platform Vsync Integration

Platforms may integrate vsync as an *input source*.

Rules:
- vsync events may dispatch `Tick`,
- vsync frequency does not affect correctness,
- headless mode ignores vsync entirely.

Vsync is optional and advisory.

---

## 12.4.9 Headless and Test Execution

In tests:

- frames are advanced manually,
- time is controlled explicitly,
- frames are inspected directly.

Example:

```rust
dispatch(Tick { dt: 16 });
let frame = snapshot();
assert_eq!(frame.time(), 16);
```

Tests never depend on real frame rates.

---

## 12.4.10 Frame Coalescing and Optimization

The runtime may coalesce frames.

Rules:
- multiple invalidations at the same time produce one frame,
- coalescing must not skip observable states,
- optimization must preserve determinism.

Coalescing is transparent to tests.

---

## 12.4.11 Debugging and Tooling

Frames are first-class debugging artifacts.

Tooling may:
- step frame by frame,
- rewind via replay,
- diff frames structurally.

Frames are stable units of observation.

---

## 12.4.12 Error Conditions

Frame advancement errors include:

- advancing time backwards,
- producing frames without a valid clock,
- inconsistent pipeline execution.

Such errors indicate runtime bugs and are fatal.

---

## 12.4.13 Summary

Deterministic frame advancement ensures:

- animations evolve predictably,
- rendering is decoupled from logic,
- tests control execution precisely,
- platforms cannot introduce timing nondeterminism.

Frames are outcomes—not drivers—of the system.

---
