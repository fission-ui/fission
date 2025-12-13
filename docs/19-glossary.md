# 19. Glossary

This glossary defines the core terminology used throughout the framework documentation.
Terms are normative: their meanings are precise and stable, and they should be used consistently in code, tests, and discussions.

---

## 19.1 Core Terms

### Core Runtime
The deterministic execution engine responsible for evaluating Core IR, managing state, performing layout, generating display lists, and producing snapshots. The Core Runtime owns semantics, time, and behavior.

---

### Core IR (Core Intermediate Representation)
A small, closed, versioned set of operations that fully describes UI structure, layout, painting, input semantics, accessibility, animation hooks, and embeds. All authoring constructs lower into Core IR.

---

### Authoring Layer
The open-world API surface used by developers (and LLMs) to describe UI. Authoring constructs are ergonomic, extensible, and compiled down into Core IR. They are not semantically authoritative.

---

### Frame
A structural UI node representing containment and layout boundaries. Frames form the primary tree structure in Core IR.

---

### Snapshot
An immutable, canonical representation of system state at a specific point in time (structure, layout, semantics, display list, etc.). Snapshots are the basis for diffing, testing, and inspection.

---

### Diff
A deterministic description of changes between two snapshots. Diffs drive incremental updates and explain why recomputation occurs.

---

### Display List
A linear, renderer-agnostic sequence of paint commands produced by the Core Runtime. Display lists are deterministic and consume layout results but do not perform layout themselves.

---

### PaintMap
A mapping between Core nodes and spans of paint commands in the display list. Used for inspection, testing, and debugging.

---

### Semantics
Explicit, structured data describing the meaning of UI elements (roles, labels, values, actions, focus). Semantics are mandatory for interactable elements and drive accessibility.

---

### Action
A descriptor representing an interaction intent. Actions are pure data (not closures) and are dispatched to reducers to mutate state deterministically.

---

### Reducer
A pure function that consumes an action and the current state and produces a new state. Reducers are the only way state changes.

---

### Determinism
The guarantee that identical inputs (state, actions, time) produce identical outputs (snapshots, diffs, display lists), independent of platform, timing, or parallelism.

---

### Instrumentation
Optional data capture and observability features (snapshots, traces, geometry queries) that are zero-cost when disabled and passive when enabled.

---

### Platform Shell
A thin adapter that hosts the Core Runtime on a specific platform (desktop, mobile, web). Shells provide IO (windows, input, surfaces) but never own UI behavior.

---

### Renderer
A backend that consumes display lists and produces pixels or GPU submissions. Renderers do not perform layout, hit testing, or semantic interpretation.

---

### Headless Execution
Running the Core Runtime without a real window or display surface. Used for CI, testing, and deterministic analysis.

---

## 19.2 Abbreviations

| Abbreviation | Meaning |
|-------------|---------|
| IR | Intermediate Representation |
| UI | User Interface |
| CI | Continuous Integration |
| IME | Input Method Editor |
| DPI | Dots Per Inch |
| GPU | Graphics Processing Unit |
| CPU | Central Processing Unit |
| WASM | WebAssembly |
| AX | Accessibility |
| SoA | Structure of Arrays |
| DSL | Domain-Specific Language |

---

## 19.3 Concept Mapping to Other Frameworks

This section maps key concepts to analogous ideas in other UI frameworks.
Mappings are approximate and provided for orientation only.

---

### Flutter

| This Framework | Flutter |
|---------------|---------|
| Authoring Layer | Widget tree |
| Core IR | RenderObject tree |
| Snapshot | Element/Render snapshots (implicit) |
| Display List | Skia display list |
| Action + Reducer | Callbacks + setState |
| Semantics | SemanticsNode |
| Deterministic Tests | Widget tests (partial) |

Key difference: determinism and testability are explicit and universal here.

---

### React / React Native

| This Framework | React |
|---------------|-------|
| Authoring Layer | JSX / Components |
| Core IR | Virtual DOM (conceptual) |
| Snapshot | Fiber state |
| Diff | Reconciliation |
| Action | Event handlers |
| Reducer | Reducers (Redux-like) |

Key difference: this framework forbids implicit effects and owns time.

---

### SwiftUI / Jetpack Compose

| This Framework | SwiftUI / Compose |
|---------------|------------------|
| Authoring Layer | Declarative views |
| Core IR | Internal view graph |
| Snapshot | Implicit state graph |
| Action | Closures |
| Determinism | Best-effort |

Key difference: this framework makes the internal graph explicit and inspectable.

---

### Traditional Immediate-Mode UI (e.g. Dear ImGui)

| This Framework | Immediate Mode UI |
|---------------|-------------------|
| Snapshot | Frame state |
| Determinism | Explicit |
| Authoring | Retained mode |
| Diffing | Structural |

Key difference: retained structure with immediate-mode testability.

---

### Game Engines

| This Framework | Game Engine |
|---------------|------------|
| Core Runtime | Game loop |
| Snapshot | World state |
| Determinism | Lockstep / replay |
| Renderer | Render backend |

Key difference: UI semantics and accessibility are first-class.

---

## Summary

This glossary exists to:

- align language across teams,
- reduce ambiguity in design discussions,
- support tooling and LLM reasoning,
- anchor long-term documentation stability.

Terms defined here should be treated as part of the framework’s public contract.
