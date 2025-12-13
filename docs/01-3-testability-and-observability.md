# 1.3 Testability and Observability

This section describes how the framework is designed to make user interfaces testable and observable by construction.
Testing and observability are not auxiliary features; they are primary design goals that shape the architecture of the system.

---

## 1.3.1 UI as Data

The framework treats UI as structured data rather than opaque pixels.

At any point in time, the runtime can expose:
- a Core IR representation,
- a semantic tree,
- a layout snapshot,
- a display list with paint ordering,
- a trace of input events and emitted actions.

These artifacts are deterministic and serializable.
Tests operate on these artifacts directly.

---

## 1.3.2 Structured Snapshots

A snapshot is a complete, immutable record of the UI state at a specific frame.

A snapshot includes:
- Core IR arena and root
- Stable node identifiers
- Layout data (rects, baselines, bounds)
- Semantics (roles, labels, actions)
- Display list and node-to-paint-span mapping
- Event and action traces

Snapshots are designed to be:
- deterministic,
- diffable,
- human-readable (with tooling),
- machine-analyzable.

---

## 1.3.3 Geometry Assertions

The framework exposes precise geometric information for every node.

Tests can assert:
- absolute and relative positions,
- sizes and aspect ratios,
- alignment and ordering,
- baselines and text metrics,
- clip and paint bounds.

Example assertions include:
- “this button is left of that label”
- “this spacer is exactly 16 logical pixels wide”
- “these two elements share a baseline”

These assertions are stable across platforms given the same configuration.

---

## 1.3.4 Semantic Assertions

Semantics are first-class and always present for interactive elements.

Tests can assert:
- roles (Button, Text, Slider, etc.),
- labels and values,
- supported actions,
- focus order and traversal.

This enables:
- accessibility testing,
- behavior verification without rendering,
- robust selectors for test queries.

---

## 1.3.5 Event and Action Traces

All input and interaction flows are traceable.

The runtime records:
- input events and their ordering,
- hit-test paths,
- emitted actions,
- reducer invocations.

Traces are deterministic and can be:
- inspected in tests,
- serialized for replay,
- used to diagnose unexpected behavior.

Tests can assert both *what* happened and *why* it happened.

---

## 1.3.6 Headless Testing

The framework supports fully headless execution.

In headless mode:
- no windows are created,
- no platform UI APIs are invoked,
- rendering uses an offscreen raster surface,
- all inputs are synthetic and controlled.

This allows:
- fast CI runs,
- reproducible failures,
- testing on systems without graphical environments.

---

## 1.3.7 Pixel Tests as an Optional Layer

Pixel-based tests are supported but intentionally secondary.

When enabled:
- rendering uses a deterministic raster backend,
- pixels can be captured and compared against golden images.

Pixel tests are recommended only for:
- final visual verification,
- regressions that cannot be expressed structurally.

Most correctness should be verified through structured assertions.

---

## 1.3.8 Optional Instrumentation

Instrumentation is optional and zero-cost when disabled.

When enabled, instrumentation:
- records snapshots and traces,
- exposes internal data structures,
- enables advanced debugging and tooling.

In production builds without instrumentation:
- no allocations are performed,
- no snapshot data is retained.

Instrumentation boundaries are explicit and controlled by configuration.

---

## 1.3.9 Debuggability and Failure Analysis

When a test fails, the framework provides:
- the full snapshot for the failing frame,
- a structured diff against the expected state,
- readable explanations of mismatches.

This reduces time spent diagnosing UI failures and enables automated analysis.

---

## 1.3.10 Summary

Testability and observability are core architectural concerns.

By modeling UI as data and exposing structured artifacts, the framework enables:
- reliable automated testing,
- accessibility verification,
- deterministic replay,
- deep introspection without sacrificing performance.

---
