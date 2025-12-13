# 9.2 Display List Structure

This section defines the internal structure of the **Display List**.
The display list is a compact, deterministic, renderer-agnostic IR describing *what to draw* and *in what order*.

The structure is intentionally minimal and closed.

---

## 9.2.1 Design Principles

The display list structure is designed to:

- be simple enough to diff and serialize,
- be expressive enough to represent all visuals,
- avoid renderer-specific abstractions,
- preserve strict ordering semantics,
- enable zero-heuristic playback.

The display list is not a scene graph.

---

## 9.2.2 Display List as a Linear IR

A display list is a **linear sequence** of operations.

Properties:
- operations are ordered,
- there is no implicit hierarchy,
- hierarchy is expressed via save/restore and transforms.

Linear structure enables:
- fast iteration,
- easy diffing,
- simple renderer backends.

---

## 9.2.3 Top-Level Structure

Conceptually:

```rust
DisplayList {
    version,
    bounds,
    ops: Vec<DisplayOp>,
}
```

- `version` defines the op semantics,
- `bounds` is the conservative union of all paint bounds,
- `ops` is the ordered operation stream.

---

## 9.2.4 DisplayOp Enumeration

Each operation is one of a closed set.

Typical categories include:
- state ops (save, restore),
- transform ops (translate, scale, rotate),
- clip ops (clip rect, clip path),
- draw ops (rect, path, text, image),
- effect ops (opacity, layer).

The exact set is versioned and minimal.

---

## 9.2.5 Explicit State Stack

State is modeled explicitly.

Rules:
- save pushes state,
- restore pops state,
- state mutations apply only within scope.

There is no implicit state inheritance.

---

## 9.2.6 Transforms

Transforms are explicit ops.

Rules:
- transforms compose in order,
- transforms affect subsequent ops,
- no implicit transforms exist.

Transforms are always affine and deterministic.

---

## 9.2.7 Clipping

Clipping is explicit and stack-based.

Rules:
- clip ops restrict subsequent drawing,
- clip regions intersect cumulatively,
- clip scopes are balanced via restore.

Unbalanced clips are structural errors.

---

## 9.2.8 Draw Operations

Draw ops describe concrete visuals.

Each draw op includes:
- geometry,
- resolved style (color, stroke, font),
- paint bounds.

Draw ops never reference layout nodes directly.

---

## 9.2.9 Paint Bounds Attachment

Every draw op carries paint bounds.

Rules:
- bounds are conservative,
- bounds are pre-clipped,
- bounds are used for damage tracking and testing.

Bounds are part of the op payload.

---

## 9.2.10 Metadata and Provenance

Optional metadata may be attached to ops:

- originating NodeId,
- source Core IR op,
- debug labels.

Metadata:
- does not affect rendering,
- is stripped in production builds if desired.

---

## 9.2.11 Display List Invariants

The display list enforces invariants:

- save/restore stacks must balance,
- clip scopes must be valid,
- ops must be self-contained,
- geometry must be finite.

Violations are deterministic errors.

---

## 9.2.12 Serialization and Stability

Display lists are serializable.

Properties:
- stable binary and/or textual format,
- versioned for forward compatibility,
- identical inputs produce identical serialized output.

Serialization enables golden tests and offline tooling.

---

## 9.2.13 Renderer Consumption Model

Renderers consume display lists by:

- iterating ops in order,
- maintaining a local state stack,
- translating ops to backend calls.

Renderers must not reorder or infer behavior.

---

## 9.2.14 Summary

The display list structure:

- is a linear, explicit IR,
- preserves strict paint order,
- separates drawing intent from execution,
- supports deterministic rendering, testing, and replay.

It is the final, authoritative description of visuals.

---
