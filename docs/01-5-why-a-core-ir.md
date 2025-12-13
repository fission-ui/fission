# 1.5 Why a Core IR (Haskell Core / LLVM IR Analogy)

This section explains why the framework is built around a small, closed Core Intermediate Representation (Core IR), and how this choice enables determinism, extensibility, and long-term maintainability.

The Core IR is the semantic backbone of the system. All higher-level constructs ultimately desugar into this representation.

---

## 1.5.1 The Problem With High-Level Widgets as a Foundation

High-level UI widgets are expressive, but they are a poor foundation for a framework core.

They tend to be:
- numerous and ever-growing,
- semantically overlapping,
- difficult to diff meaningfully,
- hard to reason about across platforms,
- tightly coupled to authoring ergonomics.

If the framework core were built directly on widgets such as `Button`, `ListView`, or `AnimatedContainer`, it would:
- become unstable over time,
- require frequent breaking changes,
- make deterministic testing difficult,
- tightly couple rendering and layout behavior to authoring APIs.

---

## 1.5.2 The Core IR Approach

Instead of treating widgets as primitives, the framework adopts a **two-level model**:

1. **Authoring Constructs**
   - Rich, ergonomic, open-world
   - Designed for developer productivity
   - Free to evolve

2. **Core IR**
   - Small, closed, stable
   - Designed for determinism and analysis
   - Rarely changes

All authoring constructs are *lowered* into Core IR before any layout, rendering, or event processing occurs.

---

## 1.5.3 Analogy: Haskell Core

In Haskell:
- Programmers write rich, expressive source code.
- The compiler desugars this code into a small Core language.
- Optimizations and reasoning happen on Core, not surface syntax.

Similarly:
- Developers write expressive UI widgets.
- The framework lowers them into Core IR.
- Layout, semantics, testing, and rendering operate on Core IR.

This separation allows the surface language to evolve without destabilizing the core semantics.

---

## 1.5.4 Analogy: LLVM IR

In LLVM:
- Many frontends (C, C++, Rust, Swift) lower into the same IR.
- LLVM IR is stable and analyzable.
- Backends target LLVM IR rather than source languages.

In this framework:
- Many widgets and patterns lower into the same Core IR.
- Core IR is the stable contract.
- Renderers and platform shells target Core IR outputs.

---

## 1.5.5 Properties of the Core IR

The Core IR is designed to have the following properties:

- **Small**: a limited number of operations
- **Closed**: new widgets do not add new core operations
- **Canonical**: equivalent authoring constructs lower to equivalent Core IR
- **Deterministic**: no hidden state or nondeterministic behavior
- **Serializable**: suitable for snapshots and replay
- **Platform-Agnostic**: no platform-specific semantics

These properties make Core IR suitable as a long-term compatibility boundary.

---

## 1.5.6 Desugaring as a First-Class Phase

Lowering into Core IR is not an implementation detail.

It is:
- explicit,
- deterministic,
- testable,
- and instrumentable.

During lowering:
- identities are assigned,
- canonical forms are enforced,
- nondeterminism is eliminated,
- provenance metadata is recorded.

Failures or bugs at higher layers can always be inspected at the Core IR level.

---

## 1.5.7 Enabling Extensibility Without Core Growth

Because the Core IR is expressive but minimal:

- new widgets are implemented as desugaring recipes,
- new interaction patterns reuse existing primitives,
- new layout abstractions lower into the same layout ops.

This avoids:
- bloating the core,
- version skew between widgets and runtime,
- complexity explosion.

Extensibility happens *above* the Core, not *inside* it.

---

## 1.5.8 Impact on Testing and Tooling

Operating on Core IR enables:
- stable structural snapshots,
- meaningful diffs between UI states,
- precise geometry and paint assertions,
- deterministic replay of events.

Tools and automated agents can reason about UI behavior without understanding every widget abstraction.

---

## 1.5.9 Long-Term Stability and Evolution

The Core IR defines the framework’s semantic contract.

As long as:
- Core IR semantics remain stable,
- lowering rules are well-defined,

the framework can:
- evolve its authoring APIs,
- add new widgets,
- introduce new renderers,
- improve performance,

without breaking existing applications or tests.

---

## 1.5.10 Summary

A small, closed Core IR:
- decouples expressiveness from stability,
- enables determinism and deep testing,
- supports multiple renderers and platforms,
- and provides a solid foundation for long-term evolution.

The Core IR is the most important design decision in the framework.

---
