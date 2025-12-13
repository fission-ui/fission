# 9.1 Core IR to Display List Compilation

This section defines how canonical Core IR, combined with layout and resolved style, is compiled into a Display List.
This compilation step is deterministic, ordered, and free of renderer-specific behavior.

Compilation answers the question: *how does structural UI intent become concrete draw operations?*

---

## 9.1.1 Inputs to Display List Compilation

Display list compilation consumes:

- canonical Core IR (post-lowering),
- a finalized Layout Snapshot,
- resolved style values,
- paint configuration (scale, color space),
- optional instrumentation configuration.

All inputs are explicit and immutable.

---

## 9.1.2 Outputs of Compilation

The output is a **Display List** consisting of:

- an ordered sequence of paint ops,
- explicit save/restore and clip ops,
- concrete geometry and style values,
- per-op paint bounds,
- optional debug and provenance metadata.

The display list is immutable and serializable.

---

## 9.1.3 Compilation as a Pure Pass

Compilation is a pure transformation.

Rules:
- no global state is accessed,
- no renderer APIs are invoked,
- no mutation of inputs occurs,
- identical inputs produce identical display lists.

Compilation can run headlessly.

---

## 9.1.4 Traversal Order

Compilation traverses Core IR in deterministic order.

Rules:
- structural pre-order traversal by default,
- child order is semantic and preserved,
- explicit z-order ops override default order,
- no hidden reordering is allowed.

Traversal order directly defines paint order.

---

## 9.1.5 Mapping Core Ops to Paint Ops

Each Core IR op maps to zero or more paint ops.

Examples (conceptual):

- `Frame` → save, translate, child ops, restore
- `PaintRect` → draw rect
- `Text` → draw text run(s)
- `Image` → draw image
- `Opacity` → save layer with alpha

Mappings are explicit and versioned.

---

## 9.1.6 Geometry Binding

Geometry is bound from the layout snapshot.

Rules:
- layout rects determine draw positions,
- paint bounds are expanded as needed,
- clip bounds introduce clip ops.

No geometry is recomputed during compilation.

---

## 9.1.7 Clipping and Save/Restore Emission

Compilation emits clip and save/restore ops explicitly.

Rules:
- clip push occurs when entering a clipped node,
- clip pop occurs on exit,
- save/restore stacks must be balanced.

Unbalanced stacks are compile-time errors.

---

## 9.1.8 Style Binding

Resolved styles are bound to paint ops.

Rules:
- styles are concrete values,
- inheritance is already resolved,
- no cascading logic occurs here.

Style binding is a simple data attachment step.

---

## 9.1.9 Paint Bounds Computation

Paint bounds are attached to each paint op.

Rules:
- bounds are conservative,
- bounds include effects (shadows, strokes),
- bounds are intersected with clip bounds.

Paint bounds are used downstream for damage tracking and testing.

---

## 9.1.10 Handling Invisible or Clipped Nodes

Nodes that are fully clipped:

- still produce structural ops if required,
- may elide paint ops safely,
- preserve traversal structure for determinism.

Elision rules are explicit and documented.

---

## 9.1.11 Error Handling

Compilation errors include:

- missing layout data,
- invalid geometry,
- unsupported style combinations,
- unbalanced clip stacks.

Errors are deterministic and reported with provenance.

---

## 9.1.12 Instrumentation Hooks

Instrumentation may observe:

- mapping decisions,
- emitted paint ops,
- paint bounds,
- traversal order.

Instrumentation does not alter output.

---

## 9.1.13 Versioning and Compatibility

The Core-to-display-list mapping is versioned.

Rules:
- mapping changes are explicit,
- backward compatibility is preserved where possible,
- tests can pin compilation versions if needed.

---

## 9.1.14 Summary

Core IR to display list compilation:

- binds structure, layout, and style,
- produces deterministic, renderer-agnostic draw intent,
- preserves order and geometry exactly,
- enables testing, diffing, and replay.

It is the bridge between UI semantics and pixels.

---
