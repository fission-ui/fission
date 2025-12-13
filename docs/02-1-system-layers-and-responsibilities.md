# 2.1 System Layers and Responsibilities

This section describes the system layers in detail and defines the responsibilities, constraints, and guarantees of each layer.
Clear ownership boundaries are essential to preserving determinism, testability, and cross-platform consistency.

---

## 2.1.1 Overview of Layers

The framework is composed of four primary layers:

1. Authoring Layer  
2. Core Runtime  
3. Rendering Backends  
4. Platform Shells  

Each layer has a single, primary responsibility and communicates with adjacent layers only through explicit data structures.

No layer may bypass another layer.

---

## 2.1.2 Authoring Layer

### Responsibility

The Authoring Layer is responsible for expressing *intent*.

It provides the APIs used by application developers to describe:
- UI structure
- visual hierarchy
- interaction intent
- state-dependent behavior

### Characteristics

- Open-world and extensible
- Struct-based widgets using `Default`
- Explicit state and action definitions
- No platform-specific concepts
- No rendering or layout logic

### Constraints

- Authoring widgets must be pure data
- No closures or side effects captured in the widget tree
- All interaction is expressed via typed actions
- Time, randomness, and IO are not accessible

### Guarantees

- Authoring code is deterministic
- Authoring code is cheap to rebuild
- Authoring constructs can evolve without breaking Core semantics

---

## 2.1.3 Core Runtime

### Responsibility

The Core Runtime defines the *meaning* of the UI.

It is responsible for:
- lowering authoring constructs into Core IR
- enforcing canonical forms
- computing layout deterministically
- constructing semantic trees
- routing input events and emitting actions
- producing snapshots and traces

### Characteristics

- Closed-world Core IR
- Deterministic execution model
- Platform-agnostic
- Fully inspectable and serializable

### Constraints

- No access to platform APIs
- No access to system clocks
- No implicit global state
- No nondeterministic behavior

### Guarantees

- Given identical inputs, produces identical outputs
- Defines the observable behavior of the UI
- Acts as the compatibility boundary of the framework

---

## 2.1.4 Rendering Backends

### Responsibility

Rendering Backends convert abstract drawing commands into pixels.

They are responsible for:
- consuming display lists
- applying transforms and clipping
- rasterizing or GPU-rendering primitives

### Characteristics

- Stateless with respect to UI logic
- Replaceable
- May target CPU or GPU

### Constraints

- Must not affect layout, semantics, or event routing
- Must behave as a pure function of inputs
- Must not introduce nondeterminism into Core artifacts

### Guarantees

- Identical display lists produce identical pixel output
- Rendering differences are isolated to rendering code paths

---

## 2.1.5 Platform Shells

### Responsibility

Platform Shells interface with the operating system.

They are responsible for:
- creating windows and surfaces
- collecting raw input events
- bridging accessibility APIs
- managing lifecycle events

### Characteristics

- Platform-specific
- Thin adapters
- Minimal logic

### Constraints

- Must not implement UI logic
- Must forward input events faithfully
- Must not mutate Core state directly

### Guarantees

- Platform differences are isolated
- Core Runtime remains platform-agnostic

---

## 2.1.6 Interaction Between Layers

The interaction between layers is strictly defined:

- Authoring Layer → Core Runtime: authoring node trees
- Core Runtime → Rendering Backend: display lists and layout snapshots
- Core Runtime → Platform Shell: semantic trees and accessibility data
- Platform Shell → Core Runtime: input events and lifecycle signals

All interactions are explicit and observable.

---

## 2.1.7 Headless Mode and Layer Composition

In headless mode:
- the Platform Shell is omitted
- a headless renderer is used
- input events are synthetic

The same Core Runtime and Authoring Layer are used.
This guarantees that tests exercise production logic.

---

## 2.1.8 Why Strict Layering Matters

Strict layering:
- enforces determinism
- prevents architectural drift
- enables independent evolution
- simplifies debugging
- supports multiple renderers and platforms

Violating layer boundaries is considered a design error.

---

## 2.1.9 Summary

Each layer in the system has:
- a clear responsibility,
- explicit constraints,
- and well-defined guarantees.

This structure ensures that the framework remains reliable, testable, and scalable as it grows.

---
