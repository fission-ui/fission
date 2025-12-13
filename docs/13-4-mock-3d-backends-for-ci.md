# 13.4 Mock 3D Backends for CI

This section defines **mock 3D backends** used for continuous integration, headless testing, and deterministic replay.
Mock backends ensure that 3D behavior can be validated without GPUs, drivers, or real-time rendering engines.

If 3D behavior cannot be tested in CI, it is not considered correct.

---

## 13.4.1 Purpose of Mock 3D Backends

Mock 3D backends exist to:

- remove GPU and driver dependencies,
- guarantee deterministic behavior,
- enable fast, headless CI execution,
- support snapshot-based testing,
- allow fault injection and edge-case validation.

Mocks are reference implementations, not approximations.

---

## 13.4.2 Backend Interface Contract

All 3D backends (real or mock) implement the same interface.

The interface includes:
- scene loading by identifier,
- camera state application,
- object transform evaluation,
- picking queries,
- render description generation,
- error reporting.

The Core depends only on this contract.

---

## 13.4.3 Deterministic Scene Evaluation

Mock backends evaluate scenes deterministically.

Rules:
- scene graphs are static data structures,
- transforms are applied in fixed order,
- floating-point behavior is pinned or replaced with fixed-point,
- evaluation results are stable across runs.

There is no frame-rate dependence.

---

## 13.4.4 Render Description, Not Pixels

Mock backends do not render pixels.

Instead, they produce a **render description**:
- visible object identifiers,
- resolved transforms,
- draw-call descriptors,
- bounding volumes.

This description is consumed by snapshots and tests.

---

## 13.4.5 Camera Simulation

Mock backends simulate cameras explicitly.

Rules:
- projection math is deterministic,
- clipping behavior is explicit,
- camera updates occur only via actions.

Camera results are snapshot-visible.

---

## 13.4.6 Picking Simulation

Picking is simulated deterministically.

Rules:
- ray casting uses pinned math,
- ambiguous results are resolved deterministically,
- picking never mutates scene state.

Picking results are returned as structured data.

---

## 13.4.7 Animation and Time Integration

Mock backends integrate with the owned clock.

Rules:
- no internal timers exist,
- animations advance only on `Tick`,
- results are identical under replay.

Time is fully controlled by tests.

---

## 13.4.8 Fault Injection and Error Simulation

Mock backends support fault injection.

Examples:
- missing assets,
- invalid scene graphs,
- transform overflows,
- picking failures.

Errors are surfaced deterministically and testable.

---

## 13.4.9 Snapshot Integration

Mock backend output is captured in snapshots.

Snapshots may include:
- visible object sets,
- camera matrices,
- bounding volumes,
- error states.

Snapshots are diffable and replayable.

---

## 13.4.10 CI Performance Characteristics

Mock backends are optimized for CI:

- no GPU usage,
- minimal allocations,
- predictable memory use,
- fast startup and teardown.

CI runtimes are stable and scalable.

---

## 13.4.11 Conformance Testing

Mock backends define the reference semantics.

Rules:
- real backends are validated against mock outputs,
- deviations are treated as bugs,
- conformance tests run in CI.

Mocks are executable specifications.

---

## 13.4.12 Security and Isolation

Mock backends execute no untrusted code.

Rules:
- no shader execution,
- no dynamic loading,
- no external IO.

CI remains secure and hermetic.

---

## 13.4.13 Summary

Mock 3D backends enable:

- deterministic 3D testing in CI,
- fast feedback without GPUs,
- reproducible bugs and regressions,
- confidence that 3D obeys Core semantics.

If it cannot be mocked, it cannot be shipped.

---
