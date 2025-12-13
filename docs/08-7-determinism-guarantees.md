# 8.7 Determinism Guarantees

This section formally defines the determinism guarantees of the layout system.
Determinism is not an emergent property; it is an explicit, enforced contract.

If two layout runs receive identical inputs, they **must** produce identical layout snapshots.

---

## 8.7.1 What Determinism Means Here

Layout determinism means:

- identical Core IR → identical geometry,
- identical state → identical layout,
- identical configuration → identical results,
- across machines, platforms, and executions.

Any deviation is considered a correctness bug.

---

## 8.7.2 Deterministic Inputs

The layout system accepts only deterministic inputs:

- canonical Core IR (post-lowering),
- resolved application state,
- explicit viewport dimensions,
- explicit scroll state,
- pinned font metrics and text shaping configuration,
- explicit rounding and precision policy.

No implicit inputs are allowed.

---

## 8.7.3 Prohibited Sources of Nondeterminism

The layout system must not depend on:

- wall-clock time,
- thread scheduling or concurrency,
- memory addresses or pointer identity,
- platform font APIs,
- GPU queries,
- floating-point nondeterminism.

Any attempt to access such sources is forbidden.

---

## 8.7.4 Numeric Determinism

Numeric determinism is enforced by:

- using fixed-width numeric types,
- avoiding platform-dependent math intrinsics,
- defining overflow and rounding behavior explicitly,
- applying rounding only at defined boundaries.

Floating-point operations are constrained and audited.

---

## 8.7.5 Traversal and Ordering Guarantees

All traversal and ordering is deterministic:

- tree traversal order is fixed and documented,
- child ordering is semantic and preserved,
- flex and grid iteration order is explicit,
- hash maps are never iterated without ordering.

There is no reliance on iteration order of unordered collections.

---

## 8.7.6 Font and Text Determinism

Text layout determinism requires:

- pinned font files and versions,
- fixed shaping engines and configuration,
- deterministic glyph metrics,
- deterministic line breaking rules.

Text layout is treated as pure computation.

---

## 8.7.7 Rounding Policy Guarantees

Rounding is:

- centralized in the layout system,
- applied at snapshot finalization,
- identical across platforms.

No downstream system may re-round geometry.

---

## 8.7.8 Scroll and Viewport Determinism

Scroll determinism is guaranteed because:

- scroll offsets are explicit state,
- content layout is independent of scroll position,
- clipping and translation are pure functions.

Scrolling never introduces reflow.

---

## 8.7.9 Validation and Assertion Strategy

The system actively enforces determinism by:

- validating all inputs,
- asserting invariants during layout,
- rejecting ambiguous constraints,
- optionally hashing snapshots for verification.

Failures are detected early and loudly.

---

## 8.7.10 Determinism in Testing and CI

Determinism enables:

- stable headless layout tests,
- snapshot comparison in CI,
- cross-platform consistency checks,
- reliable regression detection.

Flaky layout tests are considered a framework failure.

---

## 8.7.11 Interaction With Instrumentation

Instrumentation must not affect layout results.

Rules:
- instrumentation observes but does not modify data,
- side-channel state is prohibited,
- instrumentation code is isolated.

Disabling instrumentation must not change outcomes.

---

## 8.7.12 Forward Compatibility

Determinism guarantees are preserved across versions by:

- versioned snapshot formats,
- explicit migration rules,
- pinned algorithm definitions.

Breaking determinism is treated as a breaking change.

---

## 8.7.13 Summary

The layout system’s determinism guarantees are enforced by:

- strict input control,
- explicit ordering and numeric rules,
- pinned dependencies,
- aggressive validation.

Determinism is the foundation that enables testing, replay, accessibility, and trust in the system.

---
