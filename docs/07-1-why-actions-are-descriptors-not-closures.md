# 7.1 Why Actions Are Descriptors (Not Closures)

This section explains the architectural decision to model actions as **descriptors** rather than executable closures.
This choice is fundamental to determinism, testability, accessibility, and tooling.

Closures are convenient for authors, but hostile to systems-level guarantees.

---

## 7.1.1 What a Closure-Based Action Implies

A closure-based action typically:

- captures ambient state implicitly,
- executes arbitrary code at invocation time,
- depends on runtime context,
- hides control flow and side effects.

While ergonomic, this model introduces hidden complexity.

---

## 7.1.2 Determinism and Closures Are Incompatible

Closures break determinism because they may:

- capture non-deterministic data,
- depend on execution timing,
- access global or thread-local state,
- observe mutable UI internals.

Two visually identical UIs can behave differently depending on closure capture.

This is unacceptable for reproducible tests and replay.

---

## 7.1.3 Descriptor-Based Actions

An **action descriptor** is pure data.

It describes:
- *what* action may occur,
- *where* it applies,
- *what payload* it carries,
- *whether* it is enabled.

It does not describe *how* the action is executed.

Execution is delegated to reducers.

---

## 7.1.4 Separation of Intent and Effect

Descriptors separate:

- **Intent** (user wants to increment),
- **Effect** (state count increases).

This separation enables:
- centralized reasoning,
- validation before execution,
- deterministic state transitions.

Closures conflate intent and effect.

---

## 7.1.5 Accessibility Requires Descriptors

Accessibility systems require:

- enumerating available actions,
- knowing whether actions are enabled,
- invoking actions programmatically.

Closures cannot be enumerated or reasoned about safely.

Descriptors are discoverable and inspectable.

---

## 7.1.6 Testing and Replay

Descriptor-based actions enable:

- recording interaction sequences,
- replaying actions deterministically,
- asserting state transitions directly.

Closures cannot be reliably serialized or replayed.

---

## 7.1.7 Tooling and LLM Integration

Structured action descriptors allow:

- static analysis of interaction models,
- automated test generation,
- LLM-based reasoning about behavior,
- refactoring tools that preserve intent.

Closures are opaque to tooling.

---

## 7.1.8 Explicit Validation

Because descriptors are data, the system can validate:

- role/action compatibility,
- payload schemas,
- enabled/disabled state,
- routing correctness.

Closure-based systems can only fail at runtime.

---

## 7.1.9 Performance Considerations

Descriptor-based actions:

- avoid allocating closures per frame,
- reduce capture overhead,
- centralize execution paths.

Reducers are hot-path optimized and cache-friendly.

Performance is predictable.

---

## 7.1.10 Ergonomics at the Authoring Layer

Authoring ergonomics are preserved by:

- derive macros (e.g. `#[derive(Action)]`),
- concise action declarations,
- explicit reducer definitions.

Convenience is layered above the Core, not baked into it.

---

## 7.1.11 Comparison to Other Frameworks

Many frameworks use closures because:

- they optimize for immediate ergonomics,
- they assume runtime-only interaction,
- they de-prioritize testability.

This framework prioritizes:
- determinism,
- observability,
- correctness.

The trade-off is intentional.

---

## 7.1.12 Summary

Actions are descriptors, not closures, because descriptors:

- preserve determinism,
- enable accessibility by construction,
- support replay and testing,
- unlock powerful tooling,
- keep execution explicit and centralized.

This decision underpins the framework’s core guarantees.

---
