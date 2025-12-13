# 4.3.1 Structural Ops

This section defines the **Structural Operations** in the Core IR.
Structural ops define hierarchy, ownership, and traversal order.
They do **not** imply layout, rendering, or interaction behavior by themselves.

Structural ops are the backbone of the Core IR tree.

---

## 4.3.1.1 Purpose of Structural Ops

Structural ops exist to:

- define parent/child relationships,
- establish stable traversal order,
- create identity and scoping boundaries,
- group related semantic units.

They provide the skeleton on which all other semantics are layered.

---

## 4.3.1.2 Design Constraints

Structural ops must obey the following constraints:

- strictly hierarchical (tree, not graph),
- deterministic child ordering,
- no implicit behavior,
- no platform or renderer assumptions,
- stable identity boundaries.

They must be cheap to traverse and easy to canonicalize.

---

## 4.3.1.3 Core Structural Operations

The Core IR includes a small, closed set of structural operations.

### `Group`

Represents a logical grouping of child nodes.

Properties:
- ordered list of children
- no inherent layout or visual meaning
- preserves traversal order

Use cases:
- grouping semantics
- identity scoping
- lowering target for composite widgets

---

### `Scope`

Defines an explicit semantic or identity boundary.

Properties:
- single child subtree
- identity root
- boundary for state, semantics, or clipping (when combined with other ops)

Use cases:
- isolating subtrees
- ensuring identity stability
- embedding external content

---

### `Fragment`

Represents an inline structural expansion.

Properties:
- zero or more children
- no identity of its own
- transparent to traversal

Use cases:
- flattening authoring constructs
- conditional inclusion
- optional children

Fragments are eliminated during canonicalization.

---

## 4.3.1.4 Identity Semantics

Structural ops define identity behavior:

- `Group` may or may not introduce identity depending on context
- `Scope` always introduces a stable identity boundary
- `Fragment` never introduces identity

Identity assignment is deterministic and explicit.

---

## 4.3.1.5 Canonicalization Rules

Structural ops are canonicalized during lowering:

- nested `Group`s may be flattened
- empty `Group`s may be removed
- `Fragment`s are eliminated
- child ordering is normalized

Canonicalization ensures structural equivalence across builds.

---

## 4.3.1.6 Traversal Order Guarantees

Traversal order is defined as:

1. pre-order entry
2. ordered children traversal
3. post-order exit

All downstream systems (layout, semantics, paint, hit-testing) rely on this order.

Traversal order is never implicit.

---

## 4.3.1.7 Relationship to Layout and Semantics

Structural ops do not imply:

- size or position
- clipping
- visual appearance
- interaction behavior

These semantics are layered via:
- layout ops,
- semantics ops,
- rendering intent ops.

This separation preserves composability.

---

## 4.3.1.8 Failure Modes

Structural ops must be validated for:

- cycles (disallowed),
- invalid child counts,
- identity conflicts.

Invalid structural trees are rejected during lowering.

---

## 4.3.1.9 Example

Example Core IR fragment:

```text
Scope
 └─ Group
     ├─ TextNode
     └─ Group
         ├─ IconNode
         └─ LabelNode
```

This structure defines hierarchy and order only.
Layout and rendering are applied separately.

---

## 4.3.1.10 Summary

Structural ops:

- define hierarchy and traversal,
- provide identity boundaries,
- are minimal and closed,
- carry no visual or behavioral semantics.

They form the skeleton of the Core IR.

---
