# 12.5 Animation State in Snapshots

This section defines how **animation state** is captured in snapshots.
Snapshots must make all animation behavior observable, serializable, and replayable, without requiring a live runtime or renderer.

If an animation affects behavior, it must be visible in the snapshot.

---

## 12.5.1 Goals of Animation Snapshots

Animation snapshots exist to:

- expose active and completed animations,
- record timing and progress deterministically,
- enable frame-by-frame inspection,
- support replay and regression testing,
- decouple tooling from live execution.

Snapshots are the ground truth for animation behavior.

---

## 12.5.2 Snapshot Layers and Responsibilities

Animation state appears across layers:

- **Runtime State Snapshot**: authoritative animation registry and timing,
- **Layout Snapshot**: geometry after animated state is applied,
- **Paint / Display Snapshot**: visual effects derived from animation.

No layer infers animation state from another.

---

## 12.5.3 Per-Animation Snapshot Fields

Each animation snapshot entry includes:

- animation ID (stable),
- target state path,
- start time and duration,
- easing function identifier and parameters,
- current progress (0..1),
- current resolved value,
- lifecycle phase (requested, active, completed, cancelled).

All fields are explicit and versioned.

---

## 12.5.4 Global Animation Snapshot Fields

The snapshot also records:

- current owned clock value,
- reduced-motion configuration,
- animation service version,
- evaluation precision and rounding mode.

This ensures cross-version replay fidelity.

---

## 12.5.5 Completed and Historical Animations

Completed animations may be:

- retained for inspection (configurable),
- elided after completion with final value persisted,
- summarized for trace minimization.

Policies are explicit and deterministic.

---

## 12.5.6 Snapshot Determinism Guarantees

Animation snapshots are deterministic because:

- time is explicit and owned,
- progress is computed deterministically,
- easing functions are pure,
- rounding rules are fixed.

Identical inputs yield identical snapshots.

---

## 12.5.7 Snapshot Diffing and Regression Detection

Animation-aware diffs can detect:

- timing regressions,
- incorrect easing or duration,
- unintended animation triggers,
- precision or rounding changes.

Diffs operate on structured fields, not pixels.

---

## 12.5.8 Serialization and Persistence

Animation snapshots are serializable.

Use cases include:
- golden frame tests,
- offline debugging,
- bug reproduction,
- cross-machine replay.

Serialization formats are stable and forward-compatible.

---

## 12.5.9 Inspection and Tooling APIs

Inspection APIs may query:

- active animations by target,
- progress at a given time,
- lifecycle transitions,
- conflicts or overlaps.

Inspection is read-only and side-effect free.

---

## 12.5.10 Rendering Interaction

Renderers consume resolved state only.

Rules:
- renderers never read animation registry directly,
- visual output reflects snapshot-resolved values,
- rendering cannot mutate animation state.

Snapshots define the renderer contract.

---

## 12.5.11 Error States in Snapshots

Animation-related errors appear explicitly:

- invalid parameters,
- conflicting ownership,
- numeric overflow.

Errors are surfaced early and are testable.

---

## 12.5.12 Summary

Animation state in snapshots:

- makes animation behavior transparent,
- enables deterministic replay and testing,
- decouples tools from runtime execution,
- prevents hidden timing bugs.

If it animates, it snapshots.

---
