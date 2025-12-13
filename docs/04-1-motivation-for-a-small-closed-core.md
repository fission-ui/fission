# 4.1 Motivation for a Small, Closed Core

This section explains why the Core IR is intentionally small and closed, and why this constraint is fundamental to determinism, testability, and long-term evolution of the framework.

The Core IR is not a convenience layer; it is a semantic contract.

---

## 4.1.1 The Cost of Large or Open Cores

Large or extensible cores tend to accumulate problems over time:

- semantic overlap between primitives,
- inconsistent behavior across features,
- difficulty reasoning about equivalence,
- brittle testing and snapshot diffs,
- slow and risky evolution.

When the core grows freely, every new feature increases the complexity of the entire system.

---

## 4.1.2 Stability as a First-Class Requirement

The Core IR defines the behavior that applications and tests rely on.

By keeping the Core:
- small,
- closed,
- and explicit,

the framework can guarantee that:
- behavior is predictable,
- changes are deliberate,
- compatibility can be reasoned about.

Stability is not accidental; it is enforced structurally.

---

## 4.1.3 Closed World Enables Determinism

A closed-world Core IR means:

- all operations are known in advance,
- traversal and evaluation order are explicit,
- no user-defined semantics can alter core behavior.

This eliminates entire classes of nondeterminism that arise from:
- plugin-style core extensions,
- runtime-registered behavior,
- implicit semantic hooks.

---

## 4.1.4 Canonicalization and Equivalence

A small core makes canonicalization feasible.

Because the set of operations is limited:
- equivalent structures can be normalized,
- redundant forms can be eliminated,
- structural equality becomes meaningful.

This enables:
- reliable snapshot diffs,
- regression detection,
- semantic equivalence checks.

---

## 4.1.5 Separation of Innovation and Semantics

Innovation happens above the Core IR.

- New widgets
- New layout abstractions
- New interaction patterns

All of these are expressed as transformations into existing Core primitives.

This allows:
- rapid experimentation,
- multiple competing abstractions,
- evolution without semantic fragmentation.

---

## 4.1.6 Tooling and Automated Reasoning

A small, closed Core IR is tractable for tools.

Tools can:
- understand all possible node types,
- exhaustively analyze behavior,
- reason about invariants,
- generate or verify tests.

This is especially important for:
- snapshot tooling,
- accessibility validation,
- LLM-based reasoning.

---

## 4.1.7 Rendering and Platform Independence

Rendering backends and platform shells target Core IR outputs.

Because the Core IR is:
- stable,
- minimal,
- platform-agnostic,

renderers and shells:
- remain simple,
- are easier to port,
- do not need to track authoring-level changes.

---

## 4.1.8 Performance Implications

A small Core IR also benefits performance:

- fewer node types to handle,
- simpler dispatch logic,
- better cache behavior,
- easier optimization.

Performance improvements can be made centrally without altering authoring APIs.

---

## 4.1.9 Comparison to Other Systems

This approach mirrors successful systems:

- Haskell Core
- LLVM IR
- Database query planners

In each case:
- rich surface syntax lowers into a small core,
- reasoning and optimization happen at the core level,
- stability enables long-term evolution.

---

## 4.1.10 Summary

A small, closed Core IR:

- enforces determinism,
- enables canonicalization,
- stabilizes semantics,
- empowers tooling,
- supports long-term evolution.

This constraint is the foundation on which the rest of the framework is built.

---
