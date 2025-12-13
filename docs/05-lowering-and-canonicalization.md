# 5. Lowering and Canonicalization

This section describes the process by which Authoring Nodes are transformed into Core IR and normalized into a canonical form.
Lowering and canonicalization are foundational to determinism, testability, and long-term stability.

Lowering answers the question: *What does this UI mean?*  
Canonicalization answers the question: *Is this meaning expressed in a unique, comparable way?*

---

## 5.1 Role of Lowering

Lowering is the transformation from the open-world Authoring Layer into the closed-world Core IR.

Its responsibilities are to:
- translate authoring intent into Core semantics,
- eliminate authoring-level variability,
- assign stable identities,
- enforce Core IR invariants.

No layout, rendering, or event handling occurs before lowering completes.

---

## 5.2 Lowering as a Deterministic Phase

Lowering is a pure, deterministic transformation.

Given:
- the same authoring tree,
- the same application state,
- the same configuration,

lowering must always produce identical Core IR.

Lowering must not:
- access time,
- access randomness,
- access platform APIs,
- depend on memory addresses.

---

## 5.3 Lowering Pipeline Overview

Lowering proceeds in explicit stages:

1. **Authoring Node Traversal**
2. **Widget Desugaring**
3. **Structural Normalization**
4. **Identity Assignment**
5. **Core IR Emission**
6. **Canonicalization**

Each stage produces inspectable intermediate results.

---

## 5.4 Widget Desugaring

Widget desugaring translates authoring widgets into Core IR fragments.

Properties:
- pure and side-effect free,
- operates only on provided data,
- may emit multiple Core IR nodes per widget.

Desugaring:
- does not compute layout,
- does not allocate platform resources,
- does not emit rendering calls.

Custom widgets participate via the `Desugar` trait.

---

## 5.5 Structural Normalization

Before canonicalization, structural normalization enforces:

- tree shape invariants,
- explicit grouping,
- removal of empty or redundant nodes,
- flattening of transparent structures.

This ensures the Core IR structure is well-formed.

---

## 5.6 Identity Assignment

Identity is assigned during lowering, not during authoring.

Rules:
- explicit authoring IDs are honored where present,
- implicit identities are derived structurally,
- identity stability is preserved across rebuilds when structure is stable.

Identity assignment is deterministic and reproducible.

---

## 5.7 Canonicalization Goals

Canonicalization enforces a unique representation for equivalent semantics.

Its goals are to:
- eliminate semantically redundant forms,
- normalize defaults and ordering,
- make structural equality meaningful,
- enable stable snapshot diffs.

Canonicalization is not an optimization pass; it is a semantic normalization pass.

---

## 5.8 Canonicalization Rules (Examples)

Examples of canonicalization include:

- flattening nested structural groups,
- ordering children explicitly,
- making default parameters explicit,
- eliminating empty fragments,
- normalizing action and semantics ordering.

All rules are deterministic and versioned.

---

## 5.9 Versioned Canonicalization

Canonicalization rules are versioned alongside the Core IR.

This ensures:
- older Core IR retains its original meaning,
- new rules do not retroactively change semantics,
- snapshot diffs remain stable across versions.

Canonicalization is part of the Core semantic contract.

---

## 5.10 Validation and Error Handling

Lowering performs strict validation:

- structural invariants,
- identity conflicts,
- missing required semantics,
- invalid Core op usage.

Errors are:
- deterministic,
- structural,
- reported with provenance information.

Lowering failures indicate programmer error, not runtime conditions.

---

## 5.11 Inspectability and Tooling

Lowering and canonicalization are fully instrumentable.

Tools can inspect:
- pre-lowering authoring trees,
- post-desugaring Core IR,
- canonicalized Core IR.

This supports:
- debugging,
- snapshot testing,
- automated verification.

---

## 5.12 Relationship to Optimization

Lowering and canonicalization are distinct from optimization.

- Canonicalization preserves semantics exactly.
- Optimization may trade performance for equivalence.

Optimization passes, if any, operate *after* canonicalization.

---

## 5.13 Summary

Lowering and canonicalization:

- define the bridge between authoring and semantics,
- enforce determinism and stability,
- produce a unique, comparable Core IR form,
- underpin reliable testing and tooling.

They are among the most critical phases in the framework pipeline.

---
