# 5.5 Child Ordering and Structural Determinism

This section defines how child ordering is handled throughout lowering and canonicalization and why explicit, stable ordering is essential to determinism, testing, and semantic clarity.

Ordering is never implicit. If order matters, it must be represented as data.

---

## 5.5.1 Why Ordering Is a First-Class Concern

Child ordering affects:

- layout resolution,
- paint order,
- hit-testing,
- focus traversal,
- accessibility reading order,
- snapshot diffs.

If ordering is nondeterministic, the entire system becomes unreliable.

---

## 5.5.2 Explicit Ordering as a Rule

All child collections in Core IR are:

- ordered sequences,
- traversed in a documented order,
- serialized with stable ordering.

Forbidden sources of ordering include:
- hash map iteration,
- pointer order,
- insertion order from nondeterministic sources.

If an operation consumes children, it must define how order is interpreted.

---

## 5.5.3 Ordering Across Pipeline Stages

Ordering is preserved and respected across all stages:

- **Authoring Layer:** children are constructed in explicit order.
- **Desugaring:** emitted Core fragments preserve authoring order.
- **Structural Expansion:** flattening preserves relative order.
- **Canonicalization:** ordering is normalized but never randomized.

Each stage is required to maintain ordering invariants.

---

## 5.5.4 Structural Ops and Ordering

Structural ops define traversal order explicitly:

- `Group`: preserves child order as provided.
- `Scope`: has a single child; ordering is trivial.
- `Fragment`: preserves order when expanded, then eliminated.

Structural canonicalization must not reorder children.

---

## 5.5.5 Layout-Dependent Ordering

Some layout ops interpret order semantically:

- `Flex`: order defines main-axis flow.
- `Stack`: order defines z-order.
- `Grid`: order defines placement sequence when implicit.

These semantics are part of the Core IR contract and must be preserved exactly.

---

## 5.5.6 Ordering and Identity Derivation

Structural ordering participates in identity derivation.

Rules:
- sibling index is used only when no key is present,
- reordering siblings changes derived identity intentionally,
- keyed nodes decouple identity from ordering.

This makes reordering a semantic change unless keys are used.

---

## 5.5.7 Canonical Ordering Rules

Canonicalization may impose ordering only when semantics permit.

Examples:
- sorting action descriptors by stable tag,
- ordering semantic attributes by stable key,
- normalizing effect stacks.

Canonicalization must never reorder:
- structural children,
- paint ops with visual impact,
- keyed siblings.

---

## 5.5.8 Ordering and Testing

Deterministic ordering enables stable tests:

- paint lists can be snapshot-tested,
- traversal order can be asserted,
- accessibility reading order is testable.

Tests should rely on ordering only where it is semantically meaningful.

---

## 5.5.9 Error Handling and Validation

Lowering validates ordering invariants:

- duplicate keys among ordered siblings,
- illegal reordering during canonicalization,
- use of unordered collections in Core IR.

Violations are deterministic errors.

---

## 5.5.10 Summary

Explicit child ordering ensures:

- deterministic behavior across all systems,
- meaningful identity derivation,
- stable diffs and tests,
- predictable layout and interaction semantics.

Ordering is data, not an accident of implementation.

---
