# 4.3 Core IR Operation Set (Reference)

This section provides a *high-level reference* of the Core IR operation set.
It defines the categories of operations and their purpose, without going into per-operation semantics or lowering rules.

Detailed specifications for each category and operation are defined in subsections **4.3.1+**.

The goal of this section is to establish the *shape* and *scope* of the Core IR.

---

## 4.3.1 Scope of the Core IR

The Core IR represents everything required to define UI behavior:

- structure and hierarchy,
- layout constraints,
- semantics and accessibility,
- input and interaction,
- animation and time-driven changes,
- media and embedding,
- 2D and 3D composition.

It does **not** represent:
- authoring convenience,
- styling themes,
- platform APIs,
- rendering backend specifics.

---

## 4.3.2 Core IR Categories

The Core IR operation set is organized into the following categories:

1. Structural Nodes  
2. Layout and Constraint Nodes  
3. State and Property Nodes  
4. Event and Action Nodes  
5. Animation and Time Nodes  
6. Scrolling and Viewport Nodes  
7. Media and Embed Nodes  
8. Rendering Intent Nodes  
9. Accessibility and Semantics Nodes  
10. 3D and Spatial Nodes  

Each category is closed and finite.

---

## 4.3.3 Structural Nodes

Structural nodes define hierarchy and grouping.

They are responsible for:
- parent/child relationships,
- traversal order,
- identity boundaries.

Structural nodes do not imply layout or rendering behavior by themselves.

---

## 4.3.4 Layout and Constraint Nodes

Layout nodes define how space is allocated.

They represent:
- fixed and flexible sizing,
- alignment and distribution,
- grids and spanning,
- folded or multi-surface constraints.

Layout nodes produce deterministic geometry.

---

## 4.3.5 State and Property Nodes

State and property nodes represent:

- persistent values,
- derived properties,
- explicit updates.

They define *what can change over time*, not *how it is rendered*.

---

## 4.3.6 Event and Action Nodes

Event and action nodes define interaction semantics.

They represent:
- hit regions,
- supported actions,
- event routing rules.

They do not execute behavior; they emit actions.

---

## 4.3.7 Animation and Time Nodes

Animation nodes represent time-based transitions.

They define:
- animated properties,
- timing functions,
- duration and phase.

They consume the owned clock and are fully deterministic.

---

## 4.3.8 Scrolling and Viewport Nodes

Scrolling nodes define:

- scrollable regions,
- viewport clipping,
- scroll offsets and extents.

Scrolling is treated as a first-class semantic capability.

---

## 4.3.9 Media and Embed Nodes

Media nodes represent embedded content such as:

- images,
- video,
- audio,
- platform views (where supported).

Media nodes are isolated behind explicit boundaries.

---

## 4.3.10 Rendering Intent Nodes

Rendering intent nodes describe *what should be drawn*.

They represent:
- shapes,
- text runs,
- images,
- effects.

They do not encode how rendering is performed.

---

## 4.3.11 Accessibility and Semantics Nodes

Accessibility nodes define:

- semantic roles,
- labels and values,
- focus behavior,
- traversal order.

Semantics are mandatory for interactive elements.

---

## 4.3.12 3D and Spatial Nodes

3D nodes represent spatial composition.

They define:
- transforms,
- camera relationships,
- spatial hierarchies.

These nodes coexist with 2D UI in a unified Core IR.

---

## 4.3.13 Closed-World Guarantee

The operation set defined by these categories is closed.

Adding a new operation:
- requires strong justification,
- implies a semantic contract change,
- must be reviewed holistically.

Most functionality should be expressible via existing operations.

---

## 4.3.14 Relationship to Subsections

Each category and its operations are specified in detail in:

- **4.3.1+** subsections,
- with formal definitions,
- invariants,
- and examples.

This section serves as the reference map.

---

## 4.3.15 Summary

The Core IR operation set is:

- comprehensive but minimal,
- expressive but controlled,
- closed and stable.

It defines the complete semantic surface area of the framework.

---
