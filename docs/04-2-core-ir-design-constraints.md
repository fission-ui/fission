# 4.2 Core IR Design Constraints

This section defines the non-negotiable constraints that shape the Core IR.
These constraints exist to protect determinism, analyzability, and long-term stability.
Any proposed Core IR change must be evaluated against these constraints.

---

## 4.2.1 Determinism by Construction

Every Core IR operation must be deterministic.

This implies:
- no access to system time,
- no access to randomness,
- no dependency on global mutable state,
- no platform-specific behavior.

Given identical inputs, Core IR evaluation must always produce identical outputs.

---

## 4.2.2 Closed Set of Operations

The Core IR has a finite, closed set of node types and operations.

Constraints:
- new widgets must not add new Core ops,
- extensibility happens via desugaring, not Core mutation,
- Core semantics must be globally consistent.

This ensures that the Core IR remains understandable and analyzable as a whole.

---

## 4.2.3 Explicit Data, No Hidden Behavior

All Core IR behavior must be explicit in data.

Disallowed patterns include:
- implicit defaults that affect behavior,
- context-dependent interpretation,
- behavior inferred from absence of data.

Every semantic decision must be representable and inspectable.

---

## 4.2.4 Stable Identity Model

Each Core IR node must have a stable identity.

Identity must be:
- deterministic,
- independent of memory addresses,
- preserved across frames when structure is unchanged.

Identity stability underpins layout caching, snapshot diffs, and test selectors.

---

## 4.2.5 Canonical Form Requirement

Core IR must have a canonical form.

Equivalent authoring constructs must lower into:
- structurally equivalent Core IR,
- with identical ordering and grouping.

Canonicalization rules must be:
- deterministic,
- documented,
- enforced during lowering.

---

## 4.2.6 Explicit Ordering and Traversal

All ordering in Core IR is explicit.

This includes:
- child order,
- paint order,
- hit-test traversal order,
- focus traversal order.

Implicit ordering (e.g., hash map iteration) is forbidden.

---

## 4.2.7 Platform and Renderer Agnosticism

Core IR must not:
- encode platform assumptions,
- depend on renderer capabilities,
- expose OS-specific concepts.

Platform and renderer differences are handled strictly outside the Core.

---

## 4.2.8 Serialization and Diffability

Core IR must be:
- serializable,
- diffable,
- suitable for snapshot storage.

This implies:
- stable field ordering,
- versionable formats,
- avoidance of ephemeral or pointer-based data.

Serialization is a design constraint, not an afterthought.

---

## 4.2.9 Performance Predictability

Core IR operations must have predictable performance characteristics.

Constraints include:
- no unbounded recursion without explicit structure,
- no hidden allocations during evaluation,
- explicit cost models where relevant.

Performance optimizations should be possible without changing semantics.

---

## 4.2.10 Minimal Semantic Surface Area

Each Core IR operation must justify its existence.

Adding a Core op requires demonstrating:
- it cannot be expressed using existing ops,
- it represents a fundamentally new semantic capability,
- it benefits the system as a whole.

Redundancy is actively avoided.

---

## 4.2.11 Compatibility and Versioning

Core IR changes are compatibility-sensitive.

Rules:
- behavior-changing changes require versioning,
- new fields must have explicit defaults,
- old snapshots must remain interpretable when possible.

The Core IR is a long-term contract.

---

## 4.2.12 Summary

The Core IR is constrained by design.

These constraints:
- prevent accidental complexity,
- enforce determinism,
- enable deep tooling,
- protect long-term stability.

They are intentionally strict and foundational.

---
