# 18.3 Extending the Core IR Safely

This section defines how the **Core IR** may be extended over time without compromising
determinism, backwards compatibility, or the ability to reason about the system.
The Core IR is intentionally conservative; extension is possible, but rare and disciplined.

The Core grows only when reduction is impossible.

---

## 18.3.1 Why Extending the Core Is Hard

The Core IR is a **closed world** by design.

Reasons:
- all semantics depend on it,
- all determinism guarantees depend on it,
- all tooling assumes its stability,
- all renderers and shells target it.

Any Core change has system-wide impact.

---

## 18.3.2 When Core Extension Is Justified

A new Core IR operation is justified only if:

- the behavior cannot be expressed as a composition of existing ops,
- the behavior is fundamental (not stylistic or ergonomic),
- the behavior is platform-independent,
- the behavior must be reasoned about by tooling and tests,
- deferring to higher layers would break determinism or observability.

Convenience is never a sufficient reason.

---

## 18.3.3 Alternatives to Core Extension

Before extending the Core, the following must be exhausted:

- authoring-layer abstractions,
- standard library widgets,
- lowering transformations,
- renderer-specific interpretation,
- metadata or annotation channels.

Most features belong outside the Core.

---

## 18.3.4 Extension Process Overview

The extension process is formal:

1. write a design proposal,
2. prove insufficiency of existing ops,
3. define exact semantics and invariants,
4. specify interactions with all subsystems,
5. add exhaustive tests,
6. version the change explicitly.

Ad hoc changes are forbidden.

---

## 18.3.5 Versioned Core IR Evolution

Core IR extensions are versioned.

Rules:
- new ops are introduced under a new Core IR version,
- older versions remain supported,
- ops are never overloaded or repurposed,
- behavior is immutable once released.

Multiple Core IR versions may coexist.

---

## 18.3.6 Semantic Completeness Requirement

Each new op must be semantically complete.

Requirements:
- precise definition of behavior,
- deterministic execution rules,
- explicit interaction with layout, paint, input, semantics,
- defined snapshot representation,
- defined diff behavior.

Ambiguity is unacceptable.

---

## 18.3.7 Canonicalization and Normal Forms

New ops must define canonical forms.

Rules:
- equivalent constructions must canonicalize identically,
- canonicalization must be deterministic,
- elision and simplification rules must be explicit.

This preserves diffability and caching.

---

## 18.3.8 Interaction With Existing Ops

Interactions must be specified explicitly.

Questions to answer:
- how does the op compose with layout?
- how does it affect paint order?
- how does it participate in hit testing?
- how does it affect accessibility?
- how does it snapshot and diff?

Undefined interactions are not allowed.

---

## 18.3.9 Tooling and Test Impact

New Core ops must be supported by tooling.

Requirements:
- snapshot inspectors understand the op,
- test APIs can query its effects,
- diff visualizers handle it correctly,
- headless execution remains possible.

Tooling support is part of the definition.

---

## 18.3.10 Renderer Implications

Renderers must be able to consume new ops.

Rules:
- ops must lower to display list constructs,
- renderer behavior must be well-defined,
- fallback behavior must be explicit.

If renderers cannot implement it, the op is invalid.

---

## 18.3.11 Migration Strategy

Introducing a new op requires a migration story.

Options include:
- automatic lowering from old patterns,
- opt-in usage in authoring layers,
- dual support during transition.

Migrations must be deterministic.

---

## 18.3.12 Rejection Criteria

Core extensions must be rejected if:

- they primarily affect ergonomics,
- they encode stylistic conventions,
- they depend on platform behavior,
- they introduce implicit behavior,
- they complicate reasoning or testing.

Restraint is a feature.

---

## 18.3.13 Governance and Review

Core IR evolution is governed.

Practices:
- limited set of Core maintainers,
- design reviews with written rationale,
- long stabilization periods,
- explicit acceptance or rejection.

This protects long-term integrity.

---

## 18.3.14 Historical Precedent

The philosophy mirrors:
- LLVM IR evolution,
- Haskell Core stability,
- stable bytecode formats.

These systems evolve slowly—and successfully.

---

## 18.3.15 Summary

The Core IR can be extended safely because:

- extension is rare and disciplined,
- alternatives are preferred,
- semantics are explicit and versioned,
- tooling and testing are first-class concerns.

The Core remains small—not because it cannot grow,
but because it does not need to.
