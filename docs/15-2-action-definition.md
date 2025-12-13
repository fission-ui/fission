# 15.2 Action Definition

This section focuses specifically on **action definition** for the Counter application.
Actions are the sole bridge between interaction and state mutation. They are descriptors with stable identity, not executable code.

This design enables determinism, replay, accessibility parity, and precise testing.

---

## 15.2.1 Role of Actions in the Architecture

Actions serve as:

- semantic intents emitted by interactions,
- stable identifiers for accessibility and testing,
- inputs to reducers that mutate state,
- traceable events for replay and debugging.

Actions are *data*, not behavior.

---

## 15.2.2 Counter Actions

For the Counter application, a single action is sufficient.

```rust
#[derive(Action, Debug, Clone, PartialEq, Eq, Hash)]
enum CounterAction {
    Increment,
}
```

Properties:
- finite and closed set,
- no parameters for this example,
- stable identity derived at compile time.

---

## 15.2.3 `#[derive(Action)]` Semantics

The `Action` derive macro generates:

- a stable, namespaced action tag,
- hashing and equality semantics,
- optional debug metadata,
- optional versioning hooks.

The generated identity is deterministic and reproducible.

---

## 15.2.4 Stable Action Identity

Action identity must remain stable across:

- recompilations,
- refactors,
- platforms,
- test and production builds.

Stability rules:
- identity derives from the fully-qualified enum + variant,
- optional explicit overrides are supported,
- stringly-typed identifiers are avoided.

---

## 15.2.5 Why No Closures

Actions are not closures because closures:

- capture ambient state implicitly,
- are not serializable,
- cannot be replayed,
- are opaque to tooling.

By contrast, actions are transparent and inspectable.

---

## 15.2.6 Accessibility Integration

Actions are directly referenced by semantics.

```rust
Semantics {
    role: Role::Button,
    label: "Increment counter".into(),
    actions: vec![CounterAction::Increment],
}
```

Accessibility activation dispatches the same action as pointer input.
There is no parallel code path.

---

## 15.2.7 Testing Integration

Tests assert on actions directly.

```rust
click("increment_button");
assert_action_dispatched(CounterAction::Increment);
```

Tests do not simulate callbacks or inspect widget internals.

---

## 15.2.8 Action Tracing

Each dispatched action is recorded in the trace.

Trace entries include:
- action identity,
- payload (if any),
- timestamp or tick index,
- source metadata (optional).

This enables precise replay and debugging.

---

## 15.2.9 Extending Actions

As the application grows, actions scale naturally.

Examples:
```rust
enum CounterAction {
    Increment,
    Decrement,
    Reset,
}
```

Reducers evolve, but the action model remains unchanged.

---

## 15.2.10 Summary

The Counter action definition demonstrates that:

- actions are simple, explicit data,
- identity is stable and inspectable,
- accessibility, testing, and replay all converge,
- behavior lives in reducers, not widgets.

This action model is foundational to the framework’s correctness guarantees.

---
