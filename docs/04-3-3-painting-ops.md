# 4.3.3 Painting Ops

This section defines the **Painting Operations** in the Core IR.
Painting ops describe *what* should be drawn, in *what order*, and *within what bounds*.
They do not describe *how* drawing is performed; that responsibility belongs to rendering backends.

Painting ops are renderer-agnostic and deterministic.

---

## 4.3.3.1 Purpose of Painting Ops

Painting ops exist to:

- define visual primitives (text, shapes, images),
- establish deterministic paint order,
- associate drawing with layout geometry,
- support hit-testing and paint bounds.

They are the bridge between semantic structure and rendering.

---

## 4.3.3.2 Design Constraints

Painting ops must obey the following constraints:

- no direct renderer calls,
- no platform-specific concepts,
- deterministic ordering,
- explicit association with layout nodes,
- no hidden state or implicit effects.

Painting ops are pure descriptions of drawing intent.

---

## 4.3.3.3 Paint Order and Determinism

Paint order is explicit and stable.

Rules:
- paint order follows Core IR traversal order unless overridden explicitly,
- z-ordering is expressed structurally, not implicitly,
- all ordering is explicit in data.

Given the same Core IR and layout snapshot, paint order is identical.

---

## 4.3.3.4 Core Painting Operations

The Core IR includes a closed set of painting ops.

### `DrawRect`

Represents a filled or stroked rectangle.

Properties:
- geometry reference
- fill and/or stroke specification
- corner radius (optional)

Use cases:
- backgrounds
- borders
- simple shapes

---

### `DrawText`

Represents a text run.

Properties:
- text content
- font reference
- layout geometry
- baseline alignment
- color or paint reference

Use cases:
- labels
- inline text
- accessibility-visible content

---

### `DrawImage`

Represents an image draw operation.

Properties:
- image resource reference
- layout geometry
- fit and alignment rules
- sampling hints

Use cases:
- icons
- photos
- textures

---

### `DrawPath`

Represents an arbitrary vector path.

Properties:
- path geometry
- fill and/or stroke specification
- transform reference

Use cases:
- complex shapes
- icons
- custom visuals

---

## 4.3.3.5 Paint Groups and Effects

Painting ops may be grouped for effects.

### `PaintGroup`

Properties:
- ordered list of paint ops
- optional clip
- optional transform
- optional effect stack (e.g. opacity)

Use cases:
- clipping
- opacity layers
- grouped transforms

Effects are explicit and ordered.

---

## 4.3.3.6 Association With Layout

Every painting op references layout geometry:

- bounding rect
- clip bounds
- baseline (where applicable)

Painting ops do not compute geometry.
They consume geometry produced by layout ops.

---

## 4.3.3.7 Paint Bounds and Hit Testing

Painting ops define paint bounds explicitly.

These bounds are used for:
- hit-testing,
- occlusion culling,
- damage calculation,
- test assertions.

Paint bounds are deterministic and inspectable.

---

## 4.3.3.8 Canonicalization Rules

Painting ops are canonicalized during lowering:

- redundant paint groups may be flattened,
- default paints are normalized,
- ordering is made explicit.

Canonicalization ensures equivalent visuals produce equivalent paint lists.

---

## 4.3.3.9 Failure Modes

Painting ops validate:

- missing geometry references,
- invalid resource handles,
- unsupported combinations of effects.

Failures are deterministic and reported structurally.

---

## 4.3.3.10 Relationship to Rendering Backends

Rendering backends:
- consume painting ops in order,
- translate them into backend-specific calls,
- must not reorder or reinterpret semantics.

Backends are replaceable as long as they honor painting ops faithfully.

---

## 4.3.3.11 Summary

Painting ops:

- describe visual intent without rendering details,
- establish deterministic paint order,
- bind visuals to layout geometry,
- enable reliable hit-testing and testing.

They are the final semantic layer before rendering.

---
