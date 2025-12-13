# 2.2 Data Flow: Authoring → Core IR → Layout → Display List → Renderer

This section describes the end-to-end data flow through the framework.
Understanding this flow is essential for reasoning about determinism, performance, testing, and debugging.

The framework operates as a series of explicit, deterministic transformations.
Each stage consumes structured data and produces new structured data.

---

## 2.2.1 Overview of the Pipeline

The high-level pipeline is:

1. Authoring Nodes  
2. Core IR  
3. Layout Snapshot  
4. Display List  
5. Renderer Output  

Each stage is:
- explicit,
- deterministic,
- observable,
- and independently testable.

No stage mutates data produced by a later stage.

---

## 2.2.2 Authoring Nodes

### Description

Authoring nodes are the initial representation of UI intent.

They are:
- constructed using struct-based widgets,
- pure data structures,
- parameterized by application state.

Authoring nodes may be rebuilt frequently (e.g., every frame).

### Properties

- Open-world
- No platform knowledge
- No layout or rendering behavior
- No side effects

### Output

- An authoring node tree representing the desired UI structure

This representation is not used directly for layout or rendering.

---

## 2.2.3 Lowering to Core IR

### Description

Authoring nodes are lowered into the Core IR.

Lowering:
- applies deterministic desugaring rules,
- assigns stable node identities,
- enforces canonical forms,
- records provenance metadata.

### Properties

- Pure transformation
- Deterministic
- Fully inspectable

### Output

- Core IR arena (closed-world)
- Root Core node
- Identity and provenance mappings

The Core IR is the semantic contract of the framework.

---

## 2.2.4 Layout Computation

### Description

The layout engine consumes Core IR and computes geometry.

Layout determines:
- sizes and positions,
- baselines,
- paint bounds,
- clip bounds.

Layout operates in logical units and applies explicit rounding rules.

### Properties

- Deterministic
- Platform-agnostic
- Independent of rendering backend

### Output

- Layout snapshot indexed by stable node identifiers

Layout snapshots are a primary testing artifact.

---

## 2.2.5 Display List Generation

### Description

The display list is generated from:
- Core IR
- Layout snapshot

It represents an ordered list of drawing commands.

### Properties

- Deterministic paint order
- Explicit transforms and clips
- Stable mapping from nodes to draw commands

### Output

- Display list
- PaintMap (node → draw command ranges)

Display lists are renderer-agnostic.

---

## 2.2.6 Rendering

### Description

Rendering backends consume the display list and layout snapshot to produce pixels.

Rendering may use:
- CPU rasterization,
- GPU acceleration,
- platform-specific surfaces.

### Properties

- Pure function of inputs
- No side effects on Core state
- Replaceable backend

### Output

- Pixel output (onscreen or offscreen)

Rendering output is optional in tests.

---

## 2.2.7 Observability at Each Stage

Each stage can be inspected independently:

- Authoring nodes: structure and parameters
- Core IR: canonical semantic structure
- Layout snapshot: geometry and bounds
- Display list: paint order and primitives
- Renderer output: pixels (optional)

This observability is key to debugging and testing.

---

## 2.2.8 Determinism Guarantees Across the Pipeline

Determinism is enforced by:
- explicit inputs at each stage,
- no hidden state,
- owned clock,
- pinned resources,
- canonical lowering rules.

Any nondeterminism must be isolated and mocked.

---

## 2.2.9 Failure Localization

Because the pipeline is staged:

- layout bugs appear in layout snapshots,
- paint bugs appear in display lists,
- rendering bugs appear only in pixel output,
- interaction bugs appear in traces.

This makes failures easier to diagnose and fix.

---

## 2.2.10 Summary

The explicit data flow from authoring to pixels is the backbone of the framework.

By structuring UI processing as a series of deterministic transformations, the framework achieves:
- reliability,
- testability,
- cross-platform consistency,
- and long-term maintainability.

---
