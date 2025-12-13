# 6.2 Semantic Roles, Labels, and Actions

This section defines the core semantic primitives used to describe meaning and interaction: **roles**, **labels**, and **actions**.
Together, these form the minimal, sufficient vocabulary required to expose UI intent to accessibility systems, tests, and tooling.

These primitives are explicit, typed, and validated.

---

## 6.2.1 Semantic Roles

### Definition

A **SemanticRole** declares *what kind of thing* a node represents.

Examples include:
- Button
- Text
- Image
- Checkbox
- Slider
- Input
- List
- ListItem

Roles are:
- platform-agnostic,
- finite and closed-world,
- stable across framework versions.

Roles are never inferred implicitly.

---

## 6.2.2 Role Guarantees

Each role carries guarantees about:

- supported actions,
- focus behavior,
- accessibility expectations,
- test semantics.

For example:
- A `Button` role guarantees an activatable default action.
- A `Text` role guarantees no activation semantics.
- A `Slider` role guarantees a value and increment/decrement actions.

Violations are detected during validation.

---

## 6.2.3 Semantic Labels

### Definition

A **SemanticLabel** provides a human-readable description of a node.

Properties:
- localizable,
- stable across frames,
- independent of visual text.

Labels answer the question:
“What should a user hear or read when this element is focused?”

---

## 6.2.4 Label Requirements

Rules:
- interactive elements must have a label or value,
- labels must not be empty,
- multiple labels may be composed deterministically.

Labels are part of the Core IR snapshot and test surface.

---

## 6.2.5 Semantic Values

### Definition

A **SemanticValue** represents a structured value associated with a node.

Examples:
- current slider value,
- checked/unchecked state,
- text field contents.

Values are:
- typed,
- observable,
- updated via Core IR state changes.

---

## 6.2.6 Values vs Labels

Labels describe *what something is*.
Values describe *what state it is in*.

Both may coexist.

Example:
- Label: “Volume”
- Value: “75%”

Tests and accessibility tools may query either.

---

## 6.2.7 Semantic Actions

### Definition

Semantic actions describe *what can be done* to a node.

Actions are:
- declared explicitly,
- identified by stable tags,
- exposed uniformly to input systems, accessibility, and tests.

Examples:
- Activate
- Increment
- Decrement
- Submit
- Cancel

---

## 6.2.8 Actions and Roles

Roles constrain actions.

Rules:
- only valid actions may be declared for a role,
- required actions must be present,
- default actions must be explicit.

For example:
- A `Button` must declare an `Activate` action.
- A `Slider` must declare `Increment` and `Decrement`.

Invalid combinations are lowering errors.

---

## 6.2.9 Action State

Actions may have state:

- enabled / disabled,
- available / unavailable.

Action state is explicit and observable.

Disabled actions:
- remain discoverable,
- are not invokable,
- are visible to accessibility tools and tests.

---

## 6.2.10 Determinism and Observability

Roles, labels, and actions are:

- pure data,
- deterministic across runs,
- serialized in snapshots,
- queryable in headless mode.

There is no hidden behavior or inference.

---

## 6.2.11 Testing Implications

Because semantics are explicit:

```rust
find(role("slider").label("Volume"))
    .assert_value(75)
    .action("increment")
    .invoke();
```

Tests operate on meaning, not pixels.

---

## 6.2.12 Summary

Semantic roles, labels, and actions:

- define UI meaning explicitly,
- enforce valid interaction models,
- support accessibility and testing uniformly,
- form a minimal but sufficient semantic vocabulary.

They are central to the framework’s guarantee of correctness and accessibility.

---
