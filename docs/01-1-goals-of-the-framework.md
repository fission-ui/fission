# 1.1 Goals of the Framework

This section describes the concrete goals of the framework. These goals are **normative**: design decisions, APIs, and implementation details should be evaluated against them. If a proposal does not advance one or more of these goals, it should be questioned.

---

## G1. Deterministic Behavior

The framework must be deterministic by default.

Given:
- the same application code,
- the same initial state,
- the same input events,
- the same time progression,
- the same configuration (fonts, DPI, rounding rules),

the framework must produce:
- the same layout,
- the same semantic tree,
- the same display list,
- the same rendered output,
- the same event and action traces.

Determinism is required to:
- eliminate flaky tests,
- enable reliable CI,
- allow exact replay of bugs,
- make automated reasoning (including by LLMs) feasible.

All sources of nondeterminism must be explicitly controlled or isolated.

---

## G2. First-Class Testability

Testing is not an add-on; it is a primary use case.

The framework must make it easy to test:
- visual structure (layout, size, alignment),
- semantic correctness (roles, labels, actions),
- interaction behavior (events, actions, state changes),
- temporal behavior (animations, scrolling, media playback).

Tests must:
- be readable and intention-revealing,
- operate on structured data rather than pixels by default,
- run headlessly without platform dependencies.

Pixel-based tests are supported but optional.

---

## G3. Observability and Instrumentation

The framework must expose its internal model in structured form.

At runtime (and in tests), the following artifacts must be available:
- Core IR
- layout snapshots
- semantic snapshots
- display lists and paint ordering
- event and action traces

Instrumentation must be:
- optional,
- zero-cost or near-zero-cost when disabled,
- consistent across platforms.

Observability enables:
- better tests,
- better debugging,
- better tooling,
- better automated analysis.

---

## G4. Cross-Platform Consistency

The same application code must behave consistently across:
- desktop platforms,
- mobile platforms,
- web targets.

Platform-specific differences must be:
- isolated to shell layers,
- explicitly documented,
- minimized where possible.

The Core Runtime must not depend on platform APIs.

---

## G5. Rust-First and Idiomatic Design

The framework must embrace Rust’s strengths:

- strong typing,
- explicit data ownership,
- compile-time guarantees,
- predictable performance.

APIs should avoid:
- stringly-typed identifiers,
- runtime reflection,
- implicit global state.

Where dynamic behavior is required, it must be explicit and inspectable.

---

## G6. Separation of Concerns

The framework must clearly separate:
- authoring concerns (how UI is written),
- core concerns (layout, semantics, events),
- rendering concerns (pixels, GPU, Skia),
- platform concerns (windows, input, accessibility).

Each layer should:
- have a well-defined responsibility,
- be testable in isolation,
- evolve independently.

---

## G7. Extensibility Without Core Instability

The framework must support:
- new widgets,
- new layout patterns,
- new interaction models,
- new media types,

without requiring frequent changes to the Core IR.

High-level constructs must lower into a small, stable Core.

---

## G8. Accessibility by Default

Accessibility is not optional.

All interactive elements must:
- have semantic roles,
- expose labels and actions,
- participate correctly in focus traversal.

Accessibility data must be:
- derived from Core semantics,
- consistent across platforms,
- testable programmatically.

---

## G9. Support for Humans and Automated Agents

The framework must be designed for:
- human developers,
- automated testing systems,
- LLM-based agents.

This implies:
- deterministic behavior,
- structured snapshots,
- explainable failures,
- minimal reliance on opaque rendering.

The goal is not just to render UI, but to make UI behavior *reason-able*.

---

## G10. Production Readiness Without Excessive Complexity

The framework must allow teams to reach production quickly.

This requires:
- a clear mental model,
- predictable performance characteristics,
- minimal boilerplate for common cases,
- reasonable defaults that favor correctness.

Advanced features must not impose complexity on simple use cases.

---
