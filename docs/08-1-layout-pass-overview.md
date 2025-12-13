# 8.1 Layout Pass Overview

This section provides a concrete, step-by-step overview of the layout pass.
The layout pass transforms canonical Core IR into a complete, immutable layout snapshot describing geometry, alignment, and clipping.

The layout pass is deterministic, pure, and fully inspectable.

---

## 8.1.1 Purpose of the Layout Pass

The layout pass exists to:

- resolve all spatial relationships,
- compute exact geometry for every node,
- prepare data for painting, hit testing, and semantics,
- expose geometry for testing and instrumentation.

Layout does **not** perform rendering.

---

## 8.1.2 Inputs to the Layout Pass

The layout pass consumes:

- canonical Core IR,
- resolved application state,
- explicit viewport constraints,
- explicit scroll state,
- pinned font and measurement configuration,
- rounding policy (not applied yet).

No implicit inputs are permitted.

---

## 8.1.3 Outputs of the Layout Pass

The layout pass produces a **Layout Snapshot** containing:

- node rectangles (local, parent, and global),
- baselines and alignment metrics,
- paint bounds,
- hit-test bounds,
- clip and overflow regions,
- scroll-transformed geometry.

The snapshot is immutable and serializable.

---

## 8.1.4 High-Level Pass Structure

The layout pass proceeds in ordered phases:

1. Tree Traversal and Preparation
2. Constraint Resolution
3. Size Determination
4. Position Assignment
5. Overflow and Clipping Resolution
6. Snapshot Assembly

Each phase has explicit inputs and outputs.

---

## 8.1.5 Phase 1: Tree Traversal and Preparation

Responsibilities:
- traverse Core IR in stable order,
- build an internal layout tree,
- associate NodeIds with layout nodes,
- initialize constraint contexts.

Traversal order is deterministic and preserved.

---

## 8.1.6 Phase 2: Constraint Resolution

Responsibilities:
- resolve fixed, min/max, and flex constraints,
- propagate constraints down the tree,
- compute available space per node.

Constraint solving is explicit and reproducible.

---

## 8.1.7 Phase 3: Size Determination

Responsibilities:
- compute intrinsic sizes where required,
- apply constraints to derive final sizes,
- cache intrinsic measurements deterministically.

No rounding occurs in this phase.

---

## 8.1.8 Phase 4: Position Assignment

Responsibilities:
- assign positions relative to parents,
- apply layout-specific ordering rules,
- compute global coordinates.

Position assignment respects:
- child ordering,
- alignment semantics,
- scroll offsets (logically).

---

## 8.1.9 Phase 5: Overflow and Clipping

Responsibilities:
- compute overflow regions,
- apply clipping rules,
- derive paint and hit-test bounds.

Clipping does not alter layout size—only visibility.

---

## 8.1.10 Phase 6: Snapshot Assembly

Responsibilities:
- assemble final layout snapshot,
- attach geometry to NodeIds,
- validate invariants,
- prepare data for downstream consumers.

The snapshot is the sole output of layout.

---

## 8.1.11 Determinism Guarantees

The layout pass is deterministic because:

- traversal order is fixed,
- all inputs are explicit,
- numeric operations are constrained,
- rounding is deferred.

Given identical inputs, identical snapshots are produced.

---

## 8.1.12 Observability and Instrumentation

The layout pass is fully observable.

Instrumentation may:
- inspect intermediate results,
- record constraint propagation,
- capture per-node geometry.

Instrumentation is optional and zero-cost when disabled.

---

## 8.1.13 Error Handling

Layout errors include:

- unsatisfiable constraints,
- invalid geometry (negative sizes),
- inconsistent scroll extents.

Errors are:
- deterministic,
- structural,
- reported with NodeId and provenance.

---

## 8.1.14 Summary

The layout pass:

- converts semantic structure into geometry,
- is staged, explicit, and deterministic,
- produces a complete, inspectable snapshot,
- underpins rendering, input, and testing.

It is the geometric heart of the framework.

---
