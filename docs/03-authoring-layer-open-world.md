# 3. Authoring Layer (Open World)

This section describes the Authoring Layer: the public, developer-facing API used to construct user interfaces.
The Authoring Layer is intentionally open-world, ergonomic, and expressive, while remaining strictly separated from the Core Runtime.

Authoring constructs describe *intent*, not behavior.

---

## 3.1 Role of the Authoring Layer

The Authoring Layer exists to:

- express UI structure clearly and concisely,
- map application state to UI intent,
- define interaction intent via actions,
- remain pleasant and idiomatic to write in Rust.

It does **not** define:
- layout algorithms,
- rendering behavior,
- platform-specific logic,
- event routing semantics.

Those responsibilities belong to the Core Runtime.

---

## 3.2 Open-World Design

The Authoring Layer is open-world by design.

This means:
- users can define new widgets freely,
- widgets are normal Rust structs,
- no central registry of widgets exists,
- adding widgets does not require modifying the Core Runtime.

Open-world authoring enables rapid iteration and experimentation without destabilizing the system.

---

## 3.3 Struct-Based Widgets

Widgets are defined as plain Rust structs.

Example:

```rust
#[derive(Default)]
pub struct Button {
    pub id: Option<NodeId>,
    pub on_click: Option<Action>,
    pub child: Option<Node>,
    pub padding: Insets,
}
```

Key properties:
- fields are explicit,
- optional fields use `Option` or `Default`,
- construction does not require builders or macros.

This approach:
- avoids deeply nested builders,
- improves readability,
- works well with tooling and LLMs.

---

## 3.4 Use of `Default`

Widgets are designed to be constructed using `Default` plus field overrides.

Example:

```rust
Button {
    on_click: Some(CounterAction::Increment.into()),
    child: Some(Node::Text("Increment".into())),
    ..Default::default()
}
```

Benefits:
- minimal boilerplate,
- clear intent,
- easy refactoring when fields are added.

---

## 3.5 Explicit State and Actions

Authoring code does not mutate state directly.

Instead:
- state is read to build the UI,
- interactions emit typed actions,
- reducers update state outside the widget tree.

This ensures:
- deterministic rebuilds,
- clear data flow,
- testable interaction behavior.

---

## 3.6 No Closures in the Widget Tree

The Authoring Layer forbids capturing closures in widgets.

Instead of:

```rust
on_click: || state.count += 1
```

Widgets emit actions:

```rust
on_click: Some(CounterAction::Increment.into())
```

Rationale:
- closures capture hidden state,
- closures are not serializable,
- closures hinder replay and inspection.

Actions are explicit, typed, and traceable.

---

## 3.7 Authoring Nodes

All widgets ultimately lower to a common authoring node type.

Example:

```rust
pub enum Node {
    Text(Text),
    Button(Button),
    Row(Row),
    Custom(Box<dyn Desugar>),
}
```

The `Custom` variant allows extension without changing the core authoring enum.

---

## 3.8 Desugaring Responsibility

Authoring widgets must implement deterministic desugaring into Core IR.

Desugaring:
- is pure,
- produces Core primitives,
- assigns identity when needed,
- enforces canonical forms.

Authoring widgets do not perform layout or rendering.

---

## 3.9 Determinism Guarantees

The Authoring Layer must be deterministic:

- widget construction must not depend on time,
- no random values without explicit seeding,
- iteration over collections must be ordered.

Violations of these rules propagate nondeterminism downstream and are considered bugs.

---

## 3.10 Relationship to Testing

Authoring constructs are directly testable by:

- building UI trees from known state,
- inspecting lowered Core IR,
- asserting structural properties.

Tests should not need to mock authoring code; it is pure and deterministic.

---

## 3.11 Summary

The Authoring Layer is:

- expressive and ergonomic,
- open to extension,
- strictly declarative,
- free of side effects.

By keeping authoring concerns separate from semantics and rendering, the framework enables rapid development without sacrificing determinism or testability.

---
