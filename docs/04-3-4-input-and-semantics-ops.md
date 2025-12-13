# 4.3.4 Input and Semantics Ops

This section defines the **Input and Semantics Operations** in the Core IR.
These ops describe *how users interact with the UI* and *how the UI exposes meaning* to accessibility systems.
They do not execute behavior; they declare interaction and semantic intent.

Input and semantics ops are mandatory for interactive elements and are fully deterministic.

---

## 4.3.4.1 Purpose of Input and Semantics Ops

Input and semantics ops exist to:

- define hit-test regions,
- declare supported actions,
- expose semantic roles, labels, and values,
- define focus and traversal order,
- enable accessibility consistently across platforms.

They bridge structure and behavior without embedding logic.

---

## 4.3.4.2 Design Constraints

Input and semantics ops must obey the following constraints:

- no closures or executable behavior,
- no platform-specific accessibility APIs,
- deterministic hit-testing and traversal,
- explicit data for all semantics,
- mandatory semantics for interactables.

All interaction intent must be inspectable as data.

---

## 4.3.4.3 Hit Testing Ops

### `HitRegion`

Defines a region that can receive input.

Properties:
- geometry reference (from layout)
- hit-test shape (rect, rounded rect, path)
- z-order participation
- pointer pass-through rules

Use cases:
- clickable areas
- hover detection
- gesture initiation

Hit testing is deterministic and order-defined.

---

## 4.3.4.4 Action Declaration Ops

### `ActionSet`

Declares the set of actions supported by a node.

Properties:
- ordered list of action descriptors
- optional default action
- enabled/disabled state

Actions are identified by stable tags, not strings.

Use cases:
- click / press
- increment / decrement
- submit / cancel

Declaring an action does not define its behavior.

---

## 4.3.4.5 Semantic Role Ops

### `SemanticRole`

Declares the semantic role of a node.

Examples:
- Button
- Text
- Image
- Slider
- List
- ListItem

Roles are:
- explicit,
- platform-agnostic,
- used by accessibility tooling and tests.

---

## 4.3.4.6 Semantic Label and Value Ops

### `SemanticLabel`
### `SemanticValue`

Declare human-readable meaning.

Properties:
- localized string references
- optional formatting metadata

Use cases:
- screen readers
- automated tests
- inspection tooling

Labels and values are part of the Core snapshot.

---

## 4.3.4.7 Focus and Traversal Ops

### `FocusPolicy`
### `TraversalOrder`

Define keyboard and accessibility navigation.

Properties:
- focusable / not focusable
- explicit traversal ordering
- grouping behavior

Traversal order is explicit and deterministic.

---

## 4.3.4.8 Mandatory Semantics for Interactables

Any node that declares:
- a `HitRegion`, or
- an `ActionSet`,

must also declare:
- a `SemanticRole`,
- appropriate labels or values.

This ensures accessibility is not optional.

---

## 4.3.4.9 Canonicalization Rules

Input and semantics ops are canonicalized during lowering:

- default roles are made explicit,
- action sets are ordered and normalized,
- traversal order is resolved deterministically.

Canonicalization ensures consistent accessibility behavior.

---

## 4.3.4.10 Event Routing Model

Event routing proceeds as:

1. hit testing via `HitRegion`,
2. resolution of target node,
3. emission of declared actions,
4. action dispatch to reducers.

The Core Runtime never executes arbitrary callbacks.

---

## 4.3.4.11 Testing and Inspectability

Input and semantics ops enable tests such as:

- querying nodes by role or label,
- asserting available actions,
- verifying focus order,
- simulating input deterministically.

These tests do not require rendering.

---

## 4.3.4.12 Summary

Input and semantics ops:

- define interaction intent without behavior,
- enforce accessibility by construction,
- enable deterministic event routing,
- provide robust selectors for testing and tooling.

They are a foundational part of the Core IR contract.

---
