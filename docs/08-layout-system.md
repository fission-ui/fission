# 8. Layout System

This section defines the layout system: how geometry is computed deterministically from Core IR, independent of rendering, input devices, or platform quirks.
Layout is a pure function of Core structure, constraints, and state.

Layout answers one question only: *where things are*.

---

## 8.1 Goals of the Layout System

The layout system is designed to:

- be fully deterministic,
- be independent of rendering backends,
- support headless execution,
- expose complete geometry for testing,
- scale across platforms and form factors.

Layout correctness is foundational to interaction, accessibility, and testing.

---

## 8.2 Layout as a Pure Phase

Layout is a pure computation.

Given:
- canonical Core IR,
- resolved state,
- layout configuration (e.g. viewport size, rounding policy),

layout must always produce identical results.

Layout must not:
- read time,
- access platform APIs,
- allocate GPU resources,
- depend on input devices.

---

## 8.3 Layout Inputs

Layout consumes:

- Core IR structural nodes,
- layout ops and constraints,
- explicit sizes and flex parameters,
- scroll state (explicit),
- rounding policy.

No implicit data is permitted.

---

## 8.4 Layout Outputs

Layout produces a **Layout Snapshot** containing:

- absolute and relative rectangles,
- baselines and alignment metrics,
- paint bounds,
- hit-test bounds,
- overflow and clipping regions.

The snapshot is immutable and serializable.

---

## 8.5 Coordinate Spaces

The layout system operates in explicit coordinate spaces:

- local node space,
- parent-relative space,
- global/root space,
- viewport-relative space (for scrolling).

Transforms between spaces are explicit and invertible.

---

## 8.6 Constraint Solving Model

Layout constraints are solved via a deterministic model.

Supported constraint types include:
- fixed sizes,
- min/max constraints,
- flex factors,
- grid placement,
- intrinsic measurement.

Constraint resolution order is explicit and documented.

---

## 8.7 Intrinsic Measurement

Some nodes require intrinsic sizing (e.g. text).

Rules:
- intrinsic measurement is pure,
- font metrics are fixed and pinned,
- measurement results are cached deterministically.

No platform font APIs are consulted during layout.

---

## 8.8 Layout and Identity

Layout results are keyed by NodeId.

Implications:
- stable identity enables layout caching,
- diffs reflect real geometry changes,
- reordering without keys invalidates layout intentionally.

Identity stability directly affects layout performance.

---

## 8.9 Scroll Interaction

Scrolling affects layout via explicit scroll state.

Rules:
- content layout is computed at full extent,
- viewport applies clipping and translation,
- scroll offset is explicit and observable.

Scrolling never recomputes layout implicitly.

---

## 8.10 Deterministic Rounding

Rounding is applied only at defined boundaries.

Layout operates in logical units.
Final layout snapshots include rounded geometry per policy.

This ensures pixel-perfect reproducibility.

---

## 8.11 Layout Validation

Layout validation checks:

- constraint consistency,
- non-negative sizes,
- overflow correctness,
- scroll extents validity.

Failures are deterministic and reported structurally.

---

## 8.12 Testing Layout

Layout is fully testable headlessly.

Tests may assert:
- node rectangles,
- baselines,
- relative ordering,
- clipping behavior.

Example:

```rust
find(role("button")).rect().assert_eq(Rect::new(10, 20, 80, 32));
```

No rendering is required.

---

## 8.13 Relationship to Painting and Input

Layout precedes:
- painting (visual output),
- hit testing (input),
- semantics geometry.

Painting and input consume layout results but do not influence layout.

---

## 8.14 Summary

The layout system:

- is pure and deterministic,
- produces complete, inspectable geometry,
- supports scrolling and complex constraints,
- underpins testing, accessibility, and interaction.

Layout is the geometric backbone of the framework.

---
