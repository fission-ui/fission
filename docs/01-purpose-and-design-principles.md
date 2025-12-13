# 1. Purpose and Design Principles

This document defines the foundational goals and design principles of the UI framework.
All subsequent architectural decisions, APIs, and constraints flow directly from the principles described here.
If a future change conflicts with these principles, it should be considered a design regression unless explicitly justified.

---

## 1.1 Goals of the Framework

The framework is designed to support the development of modern user interfaces across:

- Desktop (Windows, macOS, Linux)
- Mobile (iOS, Android)
- Web (Canvas / WASM-based rendering)

The primary goals are:

1. **Determinism**
   - Given the same inputs, state, time, and configuration, the framework must produce the same outputs.
   - Determinism is required for reliable testing, reproducible bugs, and non-flaky CI.

2. **First-Class Testability**
   - UI behavior, layout, and rendering order must be verifiable programmatically.
   - Tests should be as easy to write as the UI itself.
   - Pixel tests are optional; structural and semantic tests are primary.

3. **Observability by Construction**
   - The framework exposes its internal model (layout, semantics, paint order, events) in structured form.
   - Instrumentation is optional but designed in from the start, not bolted on later.

4. **Cross-Platform Parity**
   - The same core logic drives all platforms.
   - Platform-specific code is isolated to thin shell layers.

5. **Rust-First Design**
   - APIs are idiomatic Rust.
   - No reliance on dynamic languages, reflection-heavy systems, or stringly-typed APIs at call sites.

6. **Human and LLM-Friendly**
   - The system is designed to be reasoned about by both humans and automated agents.
   - Structured snapshots and traces are preferred over opaque pixels.

---

## 1.2 Determinism as a First-Class Requirement

Determinism is not an optimization; it is a core requirement.

The framework must control all sources of nondeterminism, including:

- Time (the framework owns the clock)
- Fonts (bundled and pinned)
- Rounding and snapping rules
- Traversal order of trees
- Input event ordering
- Animation progression
- Randomness (if any, must be seeded and explicit)

Determinism enables:

- Reliable UI tests
- Exact replay of recorded traces
- Meaningful diffs between snapshots
- Confidence when refactoring rendering or layout code

Any subsystem that cannot be made deterministic must be isolated behind a boundary and mocked in tests.

---

## 1.3 Testability and Observability

The framework treats UI as data.

At all times, the following artifacts are available to the runtime and (optionally) to tests:

- A semantic tree (roles, labels, actions)
- A layout tree (rects, baselines, paint bounds, clip bounds)
- A core intermediate representation (Core IR)
- A display list with stable paint ordering
- Event and action traces

Tests operate on these artifacts directly.

Examples of supported assertions:

- "This button is left of that label"
- "This node has role Button and label 'Submit'"
- "This action was emitted when clicking here"
- "This node painted before that node"
- "After 200ms, this animation reached 50% progress"

This approach avoids brittle pixel-only testing while still allowing pixel tests where appropriate.

---

## 1.4 Separation of Concerns

The framework is explicitly split into layers:

1. **Authoring Layer**
   - Ergonomic, open-world API for building UI
   - Struct-based widgets using `Default`
   - No closures captured in the widget tree

2. **Core Runtime**
   - Closed-world, minimal Core IR
   - Deterministic lowering, layout, and event routing
   - Fully inspectable and snapshot-friendly

3. **Rendering Backends**
   - Skia-safe (v1)
   - Future pure-Rust renderer (v2)
   - Rendering is a pure function of Core + Layout + Resources

4. **Platform Shells**
   - Windowing, input, accessibility bridges
   - Thin and replaceable

Each layer has a clear contract and can evolve independently.

---

## 1.5 Why a Core IR (Haskell Core / LLVM IR Analogy)

High-level UI widgets are expressive but unstable as a foundation:

- They change frequently
- They are difficult to diff meaningfully
- They are hard to reason about across platforms

Instead, all authoring constructs are lowered into a small, closed **Core Intermediate Representation**.

This Core IR:

- Contains a minimal set of layout, paint, input, and semantics primitives
- Is stable over time
- Is easy to snapshot, diff, and reason about
- Allows new widgets to be added without modifying the Core

This mirrors:

- Haskell desugaring into Core
- LLVM frontends lowering into LLVM IR

The Core IR is the semantic backbone of the framework.

---

## 1.6 Non-Goals and Explicit Trade-Offs

The framework intentionally does **not** aim to:

- Mirror HTML/CSS semantics
- Expose immediate-mode rendering APIs
- Allow arbitrary side effects during rendering
- Optimize prematurely for extreme micro-performance at the cost of clarity
- Hide internal state in opaque objects

Trade-offs made explicitly:

- Slightly more upfront structure in exchange for long-term reliability
- A small closed Core instead of endless extensibility at the lowest level
- Deterministic behavior over platform-native quirks

These trade-offs are deliberate and foundational.

---
