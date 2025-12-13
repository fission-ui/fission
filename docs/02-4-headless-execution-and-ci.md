# 2.4 Headless Execution and CI

This section describes how the framework supports full headless execution and how this capability is used to enable reliable continuous integration (CI), automated testing, and tooling.

Headless execution is not a reduced or simulated mode; it exercises the same Core Runtime and rendering code paths used in production.

---

## 2.4.1 Definition of Headless Execution

Headless execution refers to running the framework without:

- creating a visible window,
- depending on platform UI toolkits,
- requiring a graphical environment.

In headless mode:
- input is synthetic and controlled,
- rendering targets offscreen surfaces,
- platform shells are replaced by a test harness.

---

## 2.4.2 Why Headless Execution Is Essential

Headless execution enables:

- fast and reliable CI pipelines,
- deterministic UI tests,
- automated regression detection,
- offline analysis and tooling,
- LLM-driven testing and verification.

Without headless support, UI testing becomes fragile, slow, and environment-dependent.

---

## 2.4.3 Core Runtime in Headless Mode

The Core Runtime operates identically in headless and non-headless modes.

Specifically:
- the same authoring code is executed,
- the same Core IR lowering occurs,
- the same layout engine is used,
- the same event routing logic applies,
- the same snapshots and traces are produced.

No conditional logic exists inside the Core Runtime for headless execution.

---

## 2.4.4 Headless Rendering

In headless mode, rendering uses an offscreen backend.

For v1:
- a Skia raster surface is created in memory,
- rendering targets a fixed-size buffer,
- device pixel ratio and color space are explicit.

Headless rendering:
- is deterministic given the same inputs,
- does not depend on GPU availability,
- can be enabled or disabled per test.

Pixel output is optional and may be skipped entirely.

---

## 2.4.5 Test Harness Responsibilities

The test harness replaces the Platform Shell in headless mode.

It is responsible for:
- providing a fixed viewport size and DPI,
- injecting input events,
- advancing the owned clock,
- triggering frame processing,
- capturing snapshots and traces.

The harness does not implement UI logic.

---

## 2.4.6 Deterministic Configuration in CI

CI environments must use pinned configuration values:

- bundled fonts only,
- fixed viewport dimensions,
- fixed DPI,
- explicit rounding policy,
- deterministic resource loading.

Configuration is treated as part of the test input set.

---

## 2.4.7 Snapshot-Based Testing in CI

CI tests primarily rely on structured snapshots rather than pixels.

Common assertions include:
- layout geometry,
- semantic roles and labels,
- action emission,
- paint ordering.

Snapshots can be:
- compared structurally,
- diffed textually,
- stored as artifacts for inspection.

This approach minimizes flakiness.

---

## 2.4.8 Optional Pixel Tests in CI

Pixel tests are supported but optional.

When enabled:
- golden images are generated deterministically,
- comparisons allow explicit tolerances if required,
- failures produce both images and structural context.

Pixel tests should be used sparingly and intentionally.

---

## 2.4.9 CI Failure Diagnostics

When a headless test fails, CI artifacts may include:

- the failing snapshot,
- a diff against the expected snapshot,
- event and action traces,
- optional pixel output.

This information is sufficient to reproduce and diagnose failures locally.

---

## 2.4.10 Tooling and Automation

Headless execution enables advanced tooling, including:

- snapshot visualizers,
- layout inspectors,
- automated UI validators,
- LLM-driven test generators.

All tooling operates on the same deterministic artifacts.

---

## 2.4.11 Summary

Headless execution is a cornerstone of the framework.

By ensuring that headless mode exercises production code paths, the framework achieves:
- reliable CI,
- deterministic testing,
- powerful tooling,
- and confidence in UI behavior across environments.

---
