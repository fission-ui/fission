# 2. High-Level Architecture Overview

This section provides a system-level view of the framework. It explains how the major components fit together, how data flows through the system, and where key responsibilities lie.

The goal is to give contributors a clear mental model before diving into detailed APIs or implementation specifics.

---

## 2.1 Architectural Overview

At a high level, the framework is structured as a unidirectional pipeline:

**Authoring → Core Runtime → Rendering Backend → Platform Shell**

Each stage transforms data into a more concrete form, with no hidden feedback loops or implicit side effects.

All observable behavior originates in the Core Runtime.

---

## 2.2 Major Components

The framework consists of the following major components:

1. **Authoring Layer**
   - User-facing APIs for constructing UI
   - Struct-based widgets and explicit state/actions

2. **Core Runtime**
   - Deterministic lowering into Core IR
   - Layout, semantics, event routing
   - Snapshot and trace generation

3. **Rendering Backends**
   - Convert display lists into pixels
   - Skia-safe backend in v1
   - Pure Rust renderer planned for v2

4. **Platform Shells**
   - OS integration (windows, input, accessibility)
   - No UI logic

Each component has a narrow, well-defined responsibility.

---

## 2.3 Data Flow Through the System

### 2.3.1 Authoring to Core

- Authoring widgets are constructed as pure data.
- Widgets are lowered into Core IR using deterministic desugaring rules.
- Stable node identities are assigned during lowering.

Output:
- Core IR arena
- Root Core node
- Provenance and identity mappings

---

### 2.3.2 Core Processing

The Core Runtime processes Core IR through several deterministic phases:

1. **Lowering and Canonicalization**
2. **Layout Computation**
3. **Semantic Tree Construction**
4. **Display List Generation**
5. **Event and Action Routing**

Each phase produces structured outputs that can be inspected or snapshotted.

---

### 2.3.3 Core to Renderer

- The Core Runtime produces a display list and layout snapshot.
- Rendering backends consume this data to produce pixels.
- Rendering is a pure function of inputs.

The renderer does not influence layout, semantics, or behavior.

---

### 2.3.4 Platform Interaction

Platform shells:
- collect input events,
- manage surfaces and lifecycle,
- expose accessibility data.

They forward events to the Core Runtime and reflect Core outputs to the OS.

---

## 2.4 Headless Execution Path

The framework supports a complete headless execution path:

- No platform shell required
- No visible window
- Deterministic offscreen rendering

This mode is used for:
- testing,
- CI,
- automated analysis,
- tooling and inspection.

Headless execution uses the same Core Runtime and rendering code paths as production.

---

## 2.5 Deterministic Frame Model

The framework operates in discrete frames.

Each frame:
1. Processes queued input events
2. Applies actions to state
3. Rebuilds authoring UI
4. Lowers to Core IR
5. Computes layout
6. Generates display list
7. Optionally renders

The frame boundary is explicit and observable.

---

## 2.6 Snapshot and Trace Production

At each frame, the Core Runtime can optionally produce:

- Core IR snapshot
- Layout snapshot
- Semantic snapshot
- Display list snapshot
- Event and action trace

Snapshots are immutable and can be stored, compared, or replayed.

---

## 2.7 Configuration and Determinism Controls

Runtime configuration includes:

- viewport size and DPI
- rounding and snapping rules
- font and resource bundles
- feature flags
- instrumentation settings

Configuration is part of the deterministic input set and must be identical for identical outputs.

---

## 2.8 Extensibility Points

The architecture supports extensibility at defined points:

- New authoring widgets via desugaring
- New rendering backends consuming display lists
- New platform shells
- Tooling built on snapshots and traces

The Core IR itself evolves slowly and deliberately.

---

## 2.9 Failure Modes and Debugging

Because all stages are explicit:

- failures can be localized to a specific phase,
- snapshots provide exact reproduction context,
- traces explain behavioral causality.

This significantly reduces debugging complexity.

---

## 2.10 Summary

The high-level architecture enforces:

- clear separation of responsibilities,
- deterministic data flow,
- strong testability and observability guarantees.

This structure enables the framework to scale across platforms, teams, and time without sacrificing reliability.

---
