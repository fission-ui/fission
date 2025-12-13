# 8.5 Baselines, Paint Bounds, and Clip Bounds

This section defines three critical geometric concepts produced by layout: **baselines**, **paint bounds**, and **clip bounds**.
These concepts are distinct, explicitly computed, and exposed for rendering, input, accessibility, and testing.

Conflating them leads to bugs; separating them enables correctness and determinism.

---

## 8.5.1 Conceptual Overview

Each layout node may expose up to three geometric artifacts:

- **Baseline**: alignment reference for text and inline content
- **Paint Bounds**: region that may be visually affected by painting
- **Clip Bounds**: region beyond which painting and hit-testing are disallowed

All three are computed during layout and stored in the layout snapshot.

---

## 8.5.2 Baselines

### Definition

A **baseline** is a horizontal reference line used to align text and inline content.

Baselines are:
- optional,
- axis-specific,
- expressed in local and global coordinates.

Not all nodes have baselines.

---

## 8.5.3 Baseline Providers

Nodes that may provide baselines include:

- text nodes,
- inline containers,
- baseline-aware layout containers (e.g. horizontal flex).

Rules:
- baseline computation is explicit,
- missing baselines are handled deterministically,
- baselines do not imply paint bounds.

---

## 8.5.4 Baseline Propagation

Baselines propagate upward according to layout semantics.

Examples:
- a flex row may expose a baseline derived from children,
- scroll containers pass baselines through unchanged,
- clipping does not alter baseline position.

Baseline propagation rules are layout-op specific and documented.

---

## 8.5.5 Paint Bounds

### Definition

**Paint bounds** define the region that a node may affect visually when painted.

Paint bounds:
- may exceed layout rects (e.g. shadows, outlines),
- include visual effects,
- are conservative (never underestimate).

Paint bounds are used for:
- rendering invalidation,
- damage tracking,
- visual testing.

---

## 8.5.6 Paint Bounds Computation

Paint bounds are computed from:

- layout rects,
- style properties (e.g. shadows, strokes),
- child paint bounds (when applicable).

Rules:
- computation is deterministic,
- no GPU queries are involved,
- rounding is deferred until final snapshot.

---

## 8.5.7 Clip Bounds

### Definition

**Clip bounds** define the region beyond which painting and hit testing are disallowed.

Clip bounds:
- are imposed by layout containers (e.g. scroll, overflow),
- restrict visibility without altering layout,
- apply transitively to descendants.

Clip bounds are always axis-aligned rectangles.

---

## 8.5.8 Clip Bounds Propagation

Clip bounds propagate downward.

Rules:
- effective clip is the intersection of ancestor clips,
- clipping does not modify paint bounds (only visibility),
- empty clip bounds short-circuit painting and hit testing.

Clip propagation is explicit and observable.

---

## 8.5.9 Relationship Between the Three

The three concepts are related but distinct:

- Baseline: alignment reference
- Paint bounds: what *could* be drawn
- Clip bounds: what *may* be drawn

A pixel is painted if and only if it lies within both paint bounds and clip bounds.

---

## 8.5.10 Interaction With Hit Testing

Hit testing uses:

- layout rects for structural targeting,
- clip bounds for visibility filtering,
- paint bounds only when explicitly requested.

This separation avoids false positives in clipped regions.

---

## 8.5.11 Rounding Rules

Rounding rules apply as follows:

- baselines: rounded only for final snapshot comparison,
- paint bounds: rounded outward (conservative),
- clip bounds: rounded inward (restrictive).

Rounding policy is centralized and deterministic.

---

## 8.5.12 Testing Geometry Artifacts

All three artifacts are testable headlessly.

Examples:

```rust
find("text").baseline().assert_eq(18);
find("card").paint_bounds().assert_contains(shadow_rect);
find("list").clip_bounds().assert_eq(viewport_rect);
```

Tests operate on layout snapshots, not rendering output.

---

## 8.5.13 Accessibility Implications

Accessibility systems consume:

- baselines for reading order (where applicable),
- clip bounds for visibility,
- paint bounds only indirectly.

Explicit geometry improves screen reader consistency.

---

## 8.5.14 Summary

Baselines, paint bounds, and clip bounds:

- serve distinct, non-overlapping purposes,
- are computed explicitly during layout,
- are deterministic and inspectable,
- support rendering, input, accessibility, and testing.

Keeping them separate is essential to a correct UI system.

---
