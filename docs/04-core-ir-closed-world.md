# 4. Core IR (Closed World)

This section introduces the Core Intermediate Representation (Core IR).
The Core IR is the semantic foundation of the framework and defines the long-term compatibility contract for UI behavior.

Unlike the Authoring Layer, the Core IR is intentionally small, closed-world, and slow to evolve.

---

## 4.1 Role of the Core IR

The Core IR exists to:

- define the meaning of UI structures and interactions,
- serve as the input to layout, semantics, and rendering,
- provide a stable target for testing and tooling,
- enforce determinism and canonical structure.

All observable UI behavior is defined in terms of Core IR.

---

## 4.2 Closed-World Design

The Core IR is closed-world by design.

This means:
- the set of Core IR operations is finite,
- new widgets do not introduce new Core ops,
- Core semantics are globally consistent.

This constraint enables:
- reliable snapshot diffs,
- stable tooling,
- long-term compatibility.

---

## 4.3 Core IR as a Semantic Contract

The Core IR defines:
- layout behavior,
- hit-testing semantics,
- accessibility semantics,
- event routing behavior,
- rendering intent.

If two versions of the framework produce identical Core IR and layout snapshots, they are semantically equivalent.

---

## 4.4 Core IR vs Authoring Nodes

Authoring Nodes:
- are open-world,
- express developer intent,
- are unstable over time.

Core IR nodes:
- are closed-world,
- express canonical semantics,
- are stable and analyzable.

Authoring Nodes are lowered into Core IR before any meaningful computation occurs.

---

## 4.5 Canonical Form

Core IR enforces canonical form.

Canonicalization includes:
- normalized child ordering,
- explicit grouping,
- elimination of redundant nodes,
- explicit identities.

Equivalent authoring trees must lower into equivalent Core IR.

---

## 4.6 Identity in the Core IR

Each Core IR node has a stable identity.

Identities are:
- deterministic,
- preserved across frames when structure is stable,
- used as keys for layout, semantics, and traces.

Identity is not pointer-based and does not depend on memory layout.

---

## 4.7 Determinism Guarantees

The Core IR guarantees determinism by construction:

- no access to time,
- no access to randomness,
- no platform APIs,
- explicit traversal order,
- explicit rounding policy inputs.

Given identical inputs, Core IR processing produces identical outputs.

---

## 4.8 Inspectability and Serialization

Core IR is designed to be:
- serializable,
- diffable,
- inspectable.

This supports:
- snapshot testing,
- replay,
- offline analysis,
- advanced tooling.

---

## 4.9 Evolution Strategy

Changes to the Core IR are rare and deliberate.

Core IR changes:
- require careful design review,
- may require versioning,
- are treated as semantic contract changes.

Most innovation occurs above the Core IR.

---

## 4.10 Summary

The Core IR is the most critical abstraction in the framework.

By keeping it small, closed, and stable, the framework achieves:
- determinism,
- testability,
- extensibility,
- and long-term maintainability.

Everything else builds on this foundation.

---
