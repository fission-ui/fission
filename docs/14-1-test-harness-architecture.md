# 14.1 Test Harness Architecture

This section describes the **test harness architecture** that underpins deterministic testing across the entire framework.
The harness is not a special runtime; it is a thin orchestration layer over the same Core systems used in production.

Tests run the real system—under explicit control.

---

## 14.1.1 Architectural Goals

The test harness must:

- execute the real Core runtime and pipelines,
- expose deterministic control over time and input,
- capture snapshots and traces efficiently,
- support headless execution in CI,
- impose zero cost when not used.

There is no separate “test-only” execution path.

---

## 14.1.2 Core Components

The harness is composed of four primary components:

1. **Runtime Instance**
   - The real Core runtime with reducers, layout, paint, and services.
2. **Driver**
   - A thin API for dispatching actions and advancing time.
3. **Snapshot Collector**
   - Captures structured snapshots after evaluation.
4. **Assertion / Query Layer**
   - Provides deterministic queries over snapshots.

Each component is optional and composable.

---

## 14.1.3 Runtime Configuration

The harness configures the runtime explicitly:

- deterministic clock enabled,
- headless renderer selected,
- mock backends registered (media, 3D, networking),
- instrumentation flags set.

Configuration is data-driven and versioned.

---

## 14.1.4 Driver API

The driver provides imperative control with deterministic semantics.

Typical operations include:
- `dispatch(action)`
- `tick(dt)`
- `advance_to(time)`
- `pump()` (evaluate pending work without time advance)

The driver never hides state changes.

---

## 14.1.5 Evaluation Cycle

A single driver step executes:

1. Action dispatch
2. Reducer evaluation
3. Animation service update (if time advanced)
4. Layout (if invalidated)
5. Display list / render description build
6. Snapshot capture (if requested)

This cycle is explicit and repeatable.

---

## 14.1.6 Snapshot Collection Strategy

Snapshots are collected:

- on demand,
- after each driver step,
- or at configured checkpoints.

Collection is cheap and incremental, supporting fine-grained tests.

---

## 14.1.7 Instrumentation Hooks

The harness may attach instrumentation hooks to:

- reducer boundaries,
- layout passes,
- paint compilation,
- backend interactions.

Hooks observe but never mutate behavior.

---

## 14.1.8 Headless Execution Model

In headless mode:

- no OS windows are created,
- no GPU context is required,
- renderers emit structured descriptions only.

Headless execution is the default for tests and CI.

---

## 14.1.9 Isolation and Repeatability

Each test harness instance is isolated.

Rules:
- no shared global state,
- deterministic resource resolution,
- explicit teardown and disposal.

Tests are order-independent.

---

## 14.1.10 Parallel Test Execution

Harness instances are safe to run in parallel.

Rules:
- no ambient singletons,
- no shared clocks,
- no global caches without namespacing.

CI scalability is preserved.

---

## 14.1.11 Failure Diagnostics

On failure, the harness provides:

- last snapshot and diff,
- action trace leading to failure,
- time and frame index,
- optional render descriptions.

Failures are explainable, not mysterious.

---

## 14.1.12 Extensibility

The harness is extensible via adapters:

- custom snapshot serializers,
- alternative query layers,
- domain-specific assertions.

Extensions must not affect Core behavior.

---

## 14.1.13 Summary

The test harness architecture succeeds because:

- it runs the real system,
- it owns time and input,
- it exposes all relevant state,
- it stays out of the Core’s way.

Tests are authoritative because they observe reality under control.

---
