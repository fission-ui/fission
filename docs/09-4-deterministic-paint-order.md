# 9.4 Deterministic Paint Order

This section defines how paint order is established and guaranteed to be deterministic.
Paint order determines visual stacking and must be stable across platforms, runs, and renderers.

Paint order is a semantic outcome of structure and explicit data—not a side effect of rendering.

---

## 9.4.1 Why Deterministic Paint Order Matters

Deterministic paint order is required to:

- ensure identical visuals across platforms,
- support reliable golden image tests,
- enable display list diffing,
- avoid renderer-dependent behavior,
- make Z-semantics explicit and testable.

Any ambiguity in paint order is a correctness failure.

---

## 9.4.2 Source of Paint Order

Paint order is derived from:

1. Canonical Core IR structure
2. Explicit child ordering
3. Explicit Z-order data (if present)
4. Deterministic compilation rules

Paint order is **not** inferred from geometry, overlap, or renderer heuristics.

---

## 9.4.3 Default Structural Ordering

By default, paint order follows a deterministic traversal:

- pre-order traversal of the Core IR tree,
- children painted in declared order,
- parents establish scope via save/restore.

This model is simple, predictable, and stable.

---

## 9.4.4 Parent–Child Ordering Rules

Rules:
- a parent’s background/content is painted before its children (unless overridden),
- children are painted fully before moving to the next sibling,
- parent overlays (if any) are explicit ops.

There is no implicit interleaving.

---

## 9.4.5 Explicit Z-Semantics

Z-ordering is modeled explicitly as data.

Examples:
- elevation layers,
- overlays,
- modal surfaces.

Rules:
- Z-order data is resolved before painting,
- ordering decisions are reflected in the display list,
- Z-order does not depend on draw-time sorting.

Z-semantics are visible and inspectable.

---

## 9.4.6 No Geometry-Based Reordering

The system must not:

- sort by Y-coordinate,
- reorder by bounding box overlap,
- infer stacking from transforms.

Such heuristics introduce nondeterminism and platform variance.

---

## 9.4.7 Save/Restore and Paint Order

Save/restore ops delimit scopes but do not affect order.

Rules:
- ops execute strictly in sequence,
- state stacks affect rendering state only,
- save/restore never reorder ops.

This keeps the IR linear and predictable.

---

## 9.4.8 Interaction With Clipping

Clipping does not alter order.

Rules:
- clip ops affect visibility only,
- clipped content is still ordered,
- paint spans remain contiguous.

Order and visibility are orthogonal concerns.

---

## 9.4.9 Overdraw and Transparency

Transparency and overdraw are resolved by order.

Rules:
- later ops visually overlay earlier ops,
- blending behavior is deterministic given order,
- no depth buffering or painter heuristics exist.

Transparency correctness depends entirely on paint order.

---

## 9.4.10 Determinism Guarantees

Paint order determinism is guaranteed because:

- traversal order is fixed,
- all ordering inputs are explicit,
- no renderer-side sorting occurs.

Identical inputs always produce identical op sequences.

---

## 9.4.11 Testing Paint Order

Paint order is testable via display lists.

Examples:

```rust
assert!(paint_map.span("background").ends_before(
    paint_map.span("content")
));
```

Tests reason about order directly, without pixels.

---

## 9.4.12 Error Handling

Ordering errors include:

- conflicting Z-order specifications,
- cycles in ordering constraints,
- ambiguous overlays.

Such errors are detected deterministically and rejected.

---

## 9.4.13 Summary

Deterministic paint order:

- is derived from structure and explicit data,
- avoids geometry- and renderer-based heuristics,
- ensures stable visuals and tests,
- makes visual stacking inspectable and reliable.

Paint order is a core semantic property, not an implementation detail.

---
