# 14.3 Geometry Assertions (rect, baseline, paint_bounds)

This section defines **geometry assertions** used in tests and tooling.
Geometry assertions allow tests to verify *what the UI looks like* using structured, deterministic data—without relying on pixels.

Geometry is observed from snapshots, not computed on demand.

---

## 14.3.1 Design Goals

Geometry assertions must:

- be deterministic across platforms,
- reflect post-layout, post-animation state,
- follow explicit rounding and precision rules,
- be explainable and diffable,
- avoid dependence on rendering backends.

Geometry is a semantic contract, not a visual guess.

---

## 14.3.2 Geometry Snapshot Source

All geometry queries operate on the **layout snapshot**.

Rules:
- layout is fully resolved before snapshotting,
- animation-resolved state is already applied,
- no lazy or incremental geometry exists.

Queries never trigger layout.

---

## 14.3.3 `rect()`: Layout Rectangle

`rect()` returns the node’s resolved layout rectangle.

Definition:
- position relative to the root coordinate space,
- width and height after constraint resolution.

Example:

```rust
let r = find("button").rect();
assert_eq!(r.x, 120);
assert_eq!(r.y, 64);
assert_eq!(r.width, 88);
assert_eq!(r.height, 32);
```

Rules:
- rects are axis-aligned,
- values use fixed precision,
- rounding rules are explicit and versioned.

---

## 14.3.4 Coordinate Space and Units

All geometry values share a common coordinate space.

Rules:
- logical pixels, not device pixels,
- origin at the root’s top-left,
- no implicit transforms applied.

Transforms are reflected in rect values.

---

## 14.3.5 `baseline()`: Text Baseline

`baseline()` returns the resolved baseline position for text-bearing nodes.

Definition:
- baseline offset relative to the node’s top edge,
- or absolute baseline position (explicitly versioned).

Example:

```rust
let b = find("label").baseline();
assert_eq!(b, 18);
```

Rules:
- baselines are font-resolved deterministically,
- pinned fonts are required for tests,
- missing baselines return explicit `None`.

Baseline behavior is fully testable.

---

## 14.3.6 Font and Text Determinism

Baseline determinism depends on font control.

Rules:
- tests use pinned font bundles,
- font metrics are versioned,
- text shaping is deterministic.

If fonts differ, baselines must differ observably.

---

## 14.3.7 `paint_bounds()`: Visual Paint Bounds

`paint_bounds()` returns the visual bounds after paint effects.

Definition:
- includes shadows, outlines, strokes, filters,
- excludes clipping applied by ancestors (unless requested).

Example:

```rust
let p = find("card").paint_bounds();
assert!(p.width > find("card").rect().width);
```

Paint bounds expose visual extent without pixels.

---

## 14.3.8 Layout Rect vs Paint Bounds

The distinction is intentional:

- `rect()` answers *where the node is laid out*,
- `paint_bounds()` answers *what pixels it could touch*.

Tests choose based on intent.

---

## 14.3.9 Interaction With Clipping

Clipping behavior is explicit.

Rules:
- `paint_bounds()` is unclipped by default,
- `paint_bounds_clipped()` may be provided explicitly,
- clip rects are snapshot-visible.

Clipping never happens implicitly.

---

## 14.3.10 Animation Effects on Geometry

Animations affect geometry deterministically.

Examples:
- size animations affect `rect()`,
- opacity animations do not,
- transform animations affect both rect and paint bounds.

Geometry reflects fully resolved animation state.

---

## 14.3.11 Error Conditions

Geometry queries may fail explicitly:

- querying `baseline()` on non-text nodes,
- querying geometry before layout,
- numeric overflow or invalid values.

Failures are deterministic and explainable.

---

## 14.3.12 Geometry Diffs and Regression Detection

Geometry assertions support structured diffs.

Diffs may reveal:
- layout regressions,
- rounding changes,
- font metric drift,
- animation timing errors.

Diffs operate on numbers, not images.

---

## 14.3.13 Summary

Geometry assertions enable precise UI verification because:

- geometry is explicit and snapshot-based,
- rounding and metrics are pinned,
- layout and paint semantics are separated,
- failures are explainable and reproducible.

If geometry cannot be asserted deterministically, the UI is not testable.

---
