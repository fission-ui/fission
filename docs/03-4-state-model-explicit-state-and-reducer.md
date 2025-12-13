# 3.4 State Model (Explicit State + Reducer)

This section defines the state management model used by the framework.
State is explicit, external to the widget tree, and updated exclusively through reducers in response to typed actions.

This model is deliberately simple, deterministic, and testable.

---

## 3.4.1 State as an Explicit Data Model

Application state is represented as plain Rust data structures.

Example:

```rust
#[derive(Clone, Debug, Default)]
pub struct CounterState {
    pub count: i32,
}
```

Key properties:
- State is owned by the application, not the UI.
- State is not stored inside widgets or nodes.
- State structures are free of framework-specific traits.

State is treated as immutable for the purposes of UI construction.

---

## 3.4.2 One-Way Data Flow

The framework enforces a strict one-way data flow:

1. State is read to construct the Authoring Node Tree.
2. User input produces typed actions.
3. Actions are dispatched to reducers.
4. Reducers update state.
5. UI is rebuilt from the new state.

There are no implicit feedback loops.

---

## 3.4.3 Actions as the Only Source of State Change

State mutations occur **only** in reducers.

Widgets and nodes:
- may emit actions,
- must not mutate state directly.

This guarantees:
- predictable behavior,
- clear causality,
- complete traceability.

---

## 3.4.4 Reducer Functions

Reducers are pure functions that update state in response to actions.

Example:

```rust
fn reduce_counter(state: &mut CounterState, action: &Action) {
    match action {
        a if a.tag == CounterAction::Increment.tag() => {
            state.count += 1;
        }
        _ => {}
    }
}
```

Reducer properties:
- deterministic,
- side-effect free,
- explicit in behavior.

Reducers may be composed or layered as needed.

---

## 3.4.5 Typed Actions and Dispatch

Actions are defined using `#[derive(Action)]`.

Example:

```rust
#[derive(Action)]
#[action(namespace = "com.example.app", stable = true)]
pub enum CounterAction {
    #[action(alias = "counter.increment")]
    Increment,
}
```

Dispatch uses stable action tags, not strings.

This avoids:
- stringly-typed APIs,
- runtime reflection,
- accidental mismatches.

---

## 3.4.6 State Ownership and Lifetime

The runtime:
- does not own application state,
- does not mutate state implicitly.

State is owned by:
- the application,
- or a higher-level app runtime.

This allows:
- multiple state stores,
- controlled lifetimes,
- deterministic replay.

---

## 3.4.7 Determinism Guarantees

The state model enforces determinism by construction:

- Reducers are pure.
- Actions are explicit and traceable.
- No hidden state exists in the UI layer.
- Time is owned by the runtime, not reducers.

Given identical action sequences, state evolution is identical.

---

## 3.4.8 Testing State Transitions

Reducers can be tested independently of the UI.

Example:

```rust
#[test]
fn increment_updates_state() {
    let mut state = CounterState::default();
    reduce_counter(&mut state, &CounterAction::Increment.into());
    assert_eq!(state.count, 1);
}
```

This supports:
- fast unit tests,
- deterministic replay,
- isolation of logic bugs.

---

## 3.4.9 Relationship to UI Rebuilds

UI rebuilds are:
- cheap,
- expected,
- deterministic.

Because the Authoring Node Tree is pure data:
- rebuilding does not leak state,
- no diffing is required at the authoring level.

Optimization happens below this layer.

---

## 3.4.10 Summary

The explicit state + reducer model provides:

- clear data flow,
- deterministic behavior,
- strong testability,
- simple mental model.

It forms the behavioral backbone of the framework and integrates cleanly with the Authoring Layer and Core Runtime.

---
