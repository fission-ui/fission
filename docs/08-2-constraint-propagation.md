# 8.2 Constraint Propagation

This section defines how layout constraints are propagated through the layout tree.
Constraint propagation determines *what space is available* to each node and is a prerequisite for deterministic size and position computation.

Constraint propagation is explicit, directional, and free of heuristics.

---

## 8.2.1 Purpose of Constraint Propagation

Constraint propagation exists to:

- communicate available space from parent to child,
- enforce min/max and fixed-size requirements,
- support flexible and intrinsic sizing,
- ensure deterministic layout outcomes.

No node may infer available space implicitly.

---

## 8.2.2 Constraint Model Overview

Each layout node operates under a **constraint context** consisting of:

- minimum size (min_width, min_height),
- maximum size (max_width, max_height),
- preferred size (optional),
- flex parameters (if applicable).

Constraints are expressed in logical units.

---

## 8.2.3 Direction of Propagation

Constraints propagate **top-down**.

Rules:
- parents determine available space for children,
- children never expand available space beyond parent constraints,
- siblings do not communicate constraints directly.

Bottom-up communication is limited to intrinsic measurement only.

---

## 8.2.4 Root Constraints

The root node receives explicit constraints derived from:

- the viewport size,
- platform shell configuration,
- test harness configuration.

Root constraints are always finite and explicit.

---

## 8.2.5 Constraint Refinement by Layout Ops

Layout ops refine constraints:

- `Flex` divides available space among children,
- `Grid` assigns cell-based constraints,
- `Fixed` overrides constraints explicitly,
- `Span` propagates constraints unchanged.

Each op defines its own refinement rules.

---

## 8.2.6 Intrinsic Measurement Interaction

Some nodes require intrinsic measurement (e.g. text).

Rules:
- intrinsic measurement occurs only when required,
- measurement is bounded by incoming constraints,
- results are cached deterministically.

Intrinsic size never violates parent constraints.

---

## 8.2.7 Min/Max Constraint Enforcement

Min/max constraints are enforced strictly.

Rules:
- final size must satisfy min ≤ size ≤ max,
- conflicting constraints are errors,
- resolution does not depend on child order.

Constraint violations are deterministic failures.

---

## 8.2.8 Flex Constraint Distribution

Flex layouts distribute remaining space:

- flex factors are normalized deterministically,
- rounding is deferred,
- zero-flex children receive intrinsic or fixed size first.

Flex distribution order is explicit and stable.

---

## 8.2.9 Constraint Saturation and Exhaustion

If available space is exhausted:

- remaining flexible children receive zero or min size,
- overflow is recorded explicitly,
- layout does not reflow unpredictably.

Overflow handling is semantic, not heuristic.

---

## 8.2.10 Determinism Guarantees

Constraint propagation is deterministic because:

- traversal order is fixed,
- arithmetic operations are well-defined,
- no heuristics or platform queries exist.

Identical inputs yield identical constraint contexts.

---

## 8.2.11 Observability and Instrumentation

Constraint propagation is observable.

Instrumentation may:
- inspect constraint contexts per node,
- trace refinement decisions,
- capture constraint conflicts.

Instrumentation does not affect outcomes.

---

## 8.2.12 Error Handling

Constraint errors include:

- unsatisfiable min/max constraints,
- infinite or NaN constraints,
- invalid flex specifications.

Errors are:
- detected during propagation,
- reported with NodeId and provenance,
- non-recoverable by default.

---

## 8.2.13 Summary

Constraint propagation:

- defines available space deterministically,
- enforces layout invariants,
- enables predictable sizing and positioning,
- supports inspection and testing.

It is the first and most critical step of layout resolution.

---
