# 1.4 Separation of Concerns

This section defines the strict separation of responsibilities within the framework.
Clear boundaries between layers are essential for determinism, testability, and long-term maintainability.

Each layer has a single primary responsibility and communicates with adjacent layers through explicit, well-defined data structures.

---

## 1.4.1 Overview of Layers

The framework is divided into four primary layers:

1. **Authoring Layer**
2. **Core Runtime**
3. **Rendering Backends**
4. **Platform Shells**

These layers form a unidirectional pipeline from intent to pixels.

---

## 1.4.2 Authoring Layer

### Responsibility
The Authoring Layer is responsible for expressing UI intent.

It provides:
- ergonomic APIs for building UI trees,
- struct-based widgets using `Default`,
- explicit state and action definitions.

### Characteristics
- Open-world: new widgets can be defined freely.
- No rendering logic.
- No platform-specific behavior.
- No side effects during construction.

### Constraints
- Authoring widgets must be pure data.
- Closures must not be captured in the widget tree.
- All behavior must be expressed via actions.

The Authoring Layer may change frequently without impacting the Core Runtime.

---

## 1.4.3 Core Runtime

### Responsibility
The Core Runtime is the semantic heart of the framework.

It is responsible for:
- lowering authoring nodes into Core IR,
- enforcing canonical forms,
- computing layout deterministically,
- routing input and actions,
- producing semantic trees and display lists.

### Characteristics
- Closed-world Core IR.
- Deterministic execution.
- Fully inspectable and snapshot-friendly.
- Platform-agnostic.

### Constraints
- No direct access to platform APIs.
- No reliance on global state.
- No nondeterministic behavior.

The Core Runtime defines the observable behavior of the UI.

---

## 1.4.4 Rendering Backends

### Responsibility
Rendering Backends convert display lists into pixels.

Examples include:
- Skia-safe backend (v1),
- future pure-Rust renderer (v2).

### Characteristics
- Stateless with respect to UI logic.
- Consume display lists and resources.
- May use GPU or CPU rendering paths.

### Constraints
- Rendering must be a pure function of:
  - display list,
  - layout snapshot,
  - resources (fonts, images),
  - rendering configuration.

Rendering backends must not affect layout, semantics, or event routing.

---

## 1.4.5 Platform Shells

### Responsibility
Platform Shells interface with the operating system.

They handle:
- window and surface creation,
- input collection,
- accessibility API integration,
- lifecycle events.

### Characteristics
- Thin adapters.
- Platform-specific.
- Replaceable.

### Constraints
- Platform shells must not implement UI logic.
- They must forward events to the Core Runtime unchanged.
- They must reflect Core semantics accurately to platform accessibility systems.

---

## 1.4.6 Communication Between Layers

Communication between layers is explicit and data-driven.

- Authoring → Core: authoring node trees
- Core → Renderer: display lists and layout snapshots
- Core → Platform: semantic trees and accessibility data
- Platform → Core: input events and lifecycle signals

No layer may bypass another layer.

---

## 1.4.7 Benefits of Strict Separation

This separation enables:
- deterministic behavior,
- independent evolution of layers,
- headless testing,
- easier debugging,
- multiple renderers and shells.

It also prevents:
- platform-specific behavior leaking into core logic,
- rendering details influencing semantics or layout,
- hidden side effects.

---

## 1.4.8 Summary

Separation of concerns is enforced structurally, not by convention.

By keeping responsibilities isolated and interactions explicit, the framework remains:
- predictable,
- testable,
- extensible,
- and robust across platforms and over time.

---
