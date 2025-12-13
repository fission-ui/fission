# 8.4 Scroll Layout Semantics

This section defines how scrolling interacts with layout.
Scrolling introduces viewport-relative coordinate spaces without compromising determinism, inspectability, or testability.

Scrolling is modeled as geometry transformation plus clipping, not as implicit re-layout.

---

## 8.4.1 Purpose of Scroll Layout

Scroll layout exists to:

- support content larger than the viewport,
- provide deterministic viewport-relative geometry,
- expose scroll state explicitly,
- decouple content layout from viewport presentation.

Scrolling never changes intrinsic content layout.

---

## 8.4.2 Scroll Containers and Viewports

A scroll container defines:

- a **viewport rectangle**,
- a **content extent**,
- a **scroll offset**,
- an axis of scrolling (horizontal, vertical, or both).

The viewport is a layout boundary with explicit semantics.

---

## 8.4.3 Content Layout Independence

Rules:
- content is laid out at full extent as if unbounded along the scroll axis,
- viewport size constrains only visibility, not layout,
- scroll offset applies a translation to content geometry.

This ensures stable content geometry across scroll positions.

---

## 8.4.4 Coordinate Spaces

Scrolling introduces a viewport-relative coordinate space.

Spaces involved:
- content-local space,
- viewport space,
- parent/global space.

Transforms between spaces are explicit and reversible.

---

## 8.4.5 Scroll Offset Semantics

Scroll offset is:

- explicit state owned by the runtime,
- expressed in logical units,
- applied as a pure translation.

Rules:
- no implicit clamping beyond declared extents,
- invalid offsets are validation errors,
- offset changes do not trigger re-layout.

---

## 8.4.6 Clipping and Visibility

Clipping is applied at the viewport boundary.

Rules:
- content outside the viewport is clipped,
- clipping affects paint and hit testing,
- clipped content still exists in layout snapshots.

Visibility is derived, not destructive.

---

## 8.4.7 Scroll Extents

Scroll extents define the valid range of scroll offsets.

Extents are computed from:
- content size,
- viewport size,
- scroll axis configuration.

Extents are explicit and observable.

---

## 8.4.8 Interaction With Flex and Grid

Scroll containers may wrap any layout.

Rules:
- inner layout computes full content size,
- scroll container applies viewport semantics,
- no special-case logic is introduced for child layouts.

Scrolling composes cleanly with other layout ops.

---

## 8.4.9 Hit Testing and Input

Hit testing uses viewport-clipped geometry.

Rules:
- input coordinates are transformed into content space,
- clipped regions do not receive hits,
- hit regions remain stable across scroll.

Hit testing is deterministic and testable.

---

## 8.4.10 Baselines and Alignment

Baseline information propagates normally through scroll containers.

Rules:
- baselines are computed in content space,
- viewport translation does not alter baseline relationships,
- baseline alignment across scroll boundaries is forbidden.

---

## 8.4.11 Rounding and Pixel Alignment

Rounding is applied after scroll translation.

Rules:
- logical geometry remains unrounded,
- translation occurs in logical space,
- final rounded geometry is stored in the layout snapshot.

This ensures pixel-perfect reproducibility.

---

## 8.4.12 Testing Scroll Layout

Scroll layout is fully testable headlessly.

Tests may assert:
- content rects independent of scroll,
- viewport rects,
- visible ranges,
- scroll extents and offsets.

Example:

```rust
assert_eq!(find("list").scroll_extent().height(), 1200);
assert_eq!(find("list").visible_rect().height(), 400);
```

---

## 8.4.13 Determinism Guarantees

Scroll layout is deterministic because:

- scroll state is explicit,
- layout is independent of scroll position,
- clipping and translation are pure.

Identical inputs produce identical layout snapshots.

---

## 8.4.14 Summary

Scroll layout semantics:

- separate content layout from viewport presentation,
- preserve determinism and inspectability,
- integrate cleanly with other layouts,
- support precise testing and accessibility.

Scrolling is a geometric transformation, not a layout side effect.

---
