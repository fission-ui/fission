# 5.1 Lowering Pipeline Overview

This section provides a concrete, step-by-step overview of the lowering pipeline.
The pipeline transforms an Authoring Node Tree into canonical Core IR in a fully deterministic and inspectable manner.

Each stage has a single responsibility and a well-defined input/output contract.

---

## 5.1.1 Pipeline Stages at a Glance

The lowering pipeline consists of the following ordered stages:

1. Authoring Tree Capture
2. Widget Desugaring
3. Structural Expansion
4. Identity Assignment
5. Core IR Emission
6. Canonicalization
7. Validation

Each stage consumes the output of the previous stage.

---

## 5.1.2 Stage 1: Authoring Tree Capture

Input:
- fully constructed Authoring Node Tree,
- application state snapshot.

Responsibilities:
- freeze the authoring tree,
- validate basic authoring invariants,
- assign traversal order.

No transformation occurs at this stage.

---

## 5.1.3 Stage 2: Widget Desugaring

Input:
- Authoring Nodes,
- lowering context.

Responsibilities:
- invoke built-in and custom widget desugaring,
- expand widgets into Core IR fragments,
- eliminate authoring-only constructs.

Output:
- an unnormalized Core IR fragment tree.

Desugaring is pure and deterministic.

---

## 5.1.4 Stage 3: Structural Expansion

Input:
- raw Core IR fragments.

Responsibilities:
- expand fragments into explicit structure,
- eliminate transparent nodes,
- enforce tree shape invariants,
- ensure explicit grouping.

Output:
- structurally valid Core IR tree.

---

## 5.1.5 Stage 4: Identity Assignment

Input:
- structurally valid Core IR tree.

Responsibilities:
- assign stable identities to all Core nodes,
- propagate explicit authoring IDs,
- derive implicit IDs deterministically,
- record identity provenance.

Identity assignment is independent of layout or rendering.

---

## 5.1.6 Stage 5: Core IR Emission

Input:
- identity-assigned Core IR tree.

Responsibilities:
- emit Core IR nodes into canonical data structures,
- attach explicit parameters and defaults,
- prepare for canonicalization.

At this stage, semantics are fully represented but not yet normalized.

---

## 5.1.7 Stage 6: Canonicalization

Input:
- emitted Core IR.

Responsibilities:
- normalize structure and ordering,
- make defaults explicit,
- eliminate redundant representations,
- enforce canonical form rules.

Canonicalization produces the definitive Core IR.

---

## 5.1.8 Stage 7: Validation

Input:
- canonical Core IR.

Responsibilities:
- validate Core invariants,
- verify semantic requirements (e.g. accessibility),
- detect invalid op combinations.

Validation failures are deterministic and structural.

---

## 5.1.9 Determinism Guarantees

The pipeline guarantees determinism because:

- all stages are pure functions,
- traversal order is explicit,
- no hidden state exists,
- no platform APIs are invoked.

Given identical inputs, the pipeline always produces identical Core IR.

---

## 5.1.10 Instrumentation and Observability

Each pipeline stage is instrumentable.

Tooling can:
- capture intermediate representations,
- attach debug metadata,
- trace provenance of Core nodes.

Instrumentation is optional and zero-cost when disabled.

---

## 5.1.11 Failure Handling Strategy

Lowering failures indicate programmer error.

Errors include:
- invalid widget configuration,
- missing required semantics,
- illegal Core IR structures.

Failures are:
- deterministic,
- reproducible,
- reported with structural context.

---

## 5.1.12 Summary

The lowering pipeline:

- is explicit and staged,
- enforces determinism and correctness,
- produces canonical Core IR,
- supports deep inspection and testing.

It is the critical bridge between authoring intent and semantic reality.

---
