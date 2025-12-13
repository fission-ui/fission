# 14.2 Selectors and Query Model

This section defines the **selector and query model** used by tests, tooling, and LLM-driven inspection.
Selectors provide a deterministic, declarative way to reference nodes and artifacts across snapshots without relying on runtime pointers or incidental structure.

Selectors are stable identifiers over *meaning*, not implementation detail.

---

## 14.2.1 Design Goals

The selector and query model must:

- be deterministic and replay-safe,
- survive refactors and reordering,
- work uniformly across structure, layout, paint, semantics, and media,
- be concise enough for human and LLM use,
- avoid stringly-typed, ad-hoc conventions where possible.

Selectors are part of the public testing API.

---

## 14.2.2 Selector Fundamentals

A selector identifies one or more nodes in a snapshot.

Selectors are:
- pure (no side effects),
- evaluated against a snapshot,
- stable across runs given identical inputs.

Selectors never depend on memory addresses or runtime IDs.

---

## 14.2.3 Selector Kinds

The framework supports multiple selector kinds.

### 14.2.3.1 Key-Based Selectors

Key selectors match explicit author-provided keys.

Examples:
```rust
find(key("submit_button"))
find(key("item_3"))
```

Keys are the strongest and most stable selector form.

---

### 14.2.3.2 Type and Role Selectors

Selectors may match by semantic role or node type.

Examples:
```rust
find(role(Role::Button))
find(type_of::<Text>())
```

These selectors are resilient to layout changes.

---

### 14.2.3.3 Path and Structural Selectors

Structural selectors match relative position in the tree.

Examples:
```rust
find("list > item[2] > label")
find(root().child(0).child(1))
```

Structural selectors are deterministic but more brittle.

---

### 14.2.3.4 Attribute-Based Selectors

Selectors may match on snapshot attributes.

Examples:
```rust
find(where_attr("enabled", true))
find(where_text("Submit"))
```

Attribute selectors operate on snapshot data only.

---

## 14.2.4 Selector Composition

Selectors are composable.

Examples:
```rust
find(role(Role::Button).and(where_text("Submit")))
find(key("list").descendant(role(Role::Item)))
```

Composition is evaluated left-to-right deterministically.

---

## 14.2.5 Query Targets

Once selected, queries operate on targets.

Targets expose query APIs for:

- structure (parent, children, siblings),
- geometry (rect, baseline, bounds),
- visibility and hit testing,
- semantics and accessibility,
- animation and media state.

Queries never mutate state.

---

## 14.2.6 Cardinality Rules

Selectors may match:
- zero nodes,
- exactly one node,
- multiple nodes.

Rules:
- `find_one` fails if cardinality ≠ 1,
- `find_all` returns a stable ordered set,
- ordering is defined by snapshot traversal order.

Ambiguity is explicit and test-visible.

---

## 14.2.7 Deterministic Ordering

When multiple nodes match:

- order is defined by canonical tree traversal,
- traversal order is stable and versioned,
- no backend-dependent ordering exists.

This guarantees reproducible query results.

---

## 14.2.8 Error Handling

Selector errors are explicit.

Errors include:
- no matches where one was required,
- ambiguous matches where singular was expected,
- invalid selector syntax.

Errors produce structured diagnostics.

---

## 14.2.9 Selector Stability Guarantees

Selectors are stable under:
- layout changes,
- rendering backend changes,
- animation progress,
- platform differences.

Selectors may break only if the *meaning* of the UI changes.

---

## 14.2.10 LLM-Friendly Design

Selectors are designed to be easy to generate and reason about.

Properties:
- small vocabulary,
- declarative structure,
- predictable failure modes.

LLMs can infer selectors from snapshots.

---

## 14.2.11 Extensibility

New selector primitives may be added via:

- namespaced extensions,
- versioned selector sets,
- backward-compatible defaults.

Core selectors remain minimal and stable.

---

## 14.2.12 Summary

The selector and query model provides:

- a stable bridge between UI meaning and tests,
- deterministic access to all observable behavior,
- a unified language for humans, CI, and LLMs.

Selectors make the UI inspectable—not fragile.

---
