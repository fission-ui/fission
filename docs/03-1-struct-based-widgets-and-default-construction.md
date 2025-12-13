# 3.1 Struct-Based Widgets and `Default` Construction

This section defines the primary authoring model of the framework: struct-based widgets constructed using `Default`.
This model is chosen to maximize clarity, extensibility, refactor safety, and compatibility with Rust tooling.

---

## 3.1.1 Rationale for Struct-Based Widgets

Widgets are defined as plain Rust structs rather than:

- inheritance hierarchies,
- builder-pattern DSLs,
- macro-heavy declarative syntax.

This approach:
- leverages Rust’s type system directly,
- avoids hidden behavior,
- produces code that is easy to read, diff, and refactor.

Widgets are data, not behavior.

---

## 3.1.2 Widget Definition Pattern

A widget is defined as a struct with public fields.

Example:

```rust
#[derive(Default)]
pub struct Text {
    pub id: Option<NodeId>,
    pub value: String,
    pub style: TextStyle,
}
```

Key characteristics:
- All configuration is explicit in fields.
- Fields are named and typed.
- Optional behavior is expressed via `Option`.

There are no required constructors or builders.

---

## 3.1.3 Use of `Default`

All widgets are expected to implement `Default`.

This enables construction using the struct update syntax:

```rust
Text {
    value: "Hello".into(),
    ..Default::default()
}
```

Benefits:
- Minimal boilerplate for common cases.
- Adding new fields does not break existing code.
- Defaults encode framework-recommended behavior.

---

## 3.1.4 Optional Fields and Extensibility

Optional configuration is expressed using:

- `Option<T>` for presence/absence,
- default values for common behavior.

Example:

```rust
#[derive(Default)]
pub struct Padding {
    pub id: Option<NodeId>,
    pub insets: Insets,
    pub child: Option<Node>,
}
```

This avoids:
- constructor explosion,
- multiple widget variants for minor differences.

---

## 3.1.5 Composition Over Inheritance

Widgets compose by containing other widgets or nodes.

Example:

```rust
Row {
    children: vec![
        Node::Text(Text { value: "A".into(), ..Default::default() }),
        Node::Text(Text { value: "B".into(), ..Default::default() }),
    ],
    ..Default::default()
}
```

There is no widget inheritance.
Behavioral reuse is achieved through composition and desugaring.

---

## 3.1.6 Identity and Optional IDs

Widgets may optionally declare an identity:

```rust
pub id: Option<NodeId>
```

If present:
- the ID becomes a stable selector target,
- identity is preserved across rebuilds.

If absent:
- identity is derived structurally during lowering.

This allows developers to opt into explicit identity only when needed.

---

## 3.1.7 Construction Ergonomics

The combination of:
- public fields,
- `Default`,
- struct update syntax,

results in concise and readable UI code without macros:

```rust
Button {
    on_click: Some(CounterAction::Increment.into()),
    child: Some(Node::Text(Text {
        value: "Increment".into(),
        ..Default::default()
    })),
    ..Default::default()
}
```

This style scales well to large trees.

---

## 3.1.8 Interaction With Tooling and LLMs

Struct-based widgets are:
- easy for IDEs to autocomplete,
- easy for formatters to format,
- easy for LLMs to generate and modify.

Because configuration is explicit and typed, automated tools can reason about UI structure reliably.

---

## 3.1.9 Determinism Considerations

Widget construction must be deterministic.

Specifically:
- iteration over collections must be ordered,
- no access to time or randomness,
- no IO during construction.

Violations propagate nondeterminism into the Core Runtime.

---

## 3.1.10 Summary

Struct-based widgets with `Default` construction provide:

- clarity and explicitness,
- refactor safety,
- minimal boilerplate,
- strong tooling support.

They form a stable, ergonomic foundation for the open-world Authoring Layer.

---
