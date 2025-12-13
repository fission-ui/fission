# 15.1 Authoring Code

This section presents the **authoring-layer code** for the Counter application.
The example demonstrates struct-based widgets with `Default`, explicit keys, declarative action wiring, and explicit state binding—without closures in the widget tree.

The authoring layer is open-world and ergonomic, while remaining fully desugarable to Core IR.

---

## 15.1.1 State Definition

Application state is explicit and serializable.

```rust
#[derive(Default)]
struct CounterState {
    value: i32,
}
```

State contains no UI concerns and no implicit behavior.

---

## 15.1.2 Actions

Actions are descriptors, not callbacks.

```rust
#[derive(Action, Debug, Clone, PartialEq, Eq, Hash)]
enum CounterAction {
    Increment,
}
```

Notes:
- `#[derive(Action)]` assigns a stable identity.
- Actions are hashable and replayable.
- No closures are captured.

---

## 15.1.3 Reducer

Reducers are pure and deterministic.

```rust
fn counter_reducer(state: &mut CounterState, action: CounterAction) {
    match action {
        CounterAction::Increment => {
            state.value += 1;
        }
    }
}
```

The reducer owns all state mutation.

---

## 15.1.4 Widget Structs

Widgets are plain structs with `Default`.

```rust
#[derive(Default)]
struct CounterView {
    pub key: Option<Key>,
    pub children: Vec<Node>,
}
```

Widgets do not execute logic; they declare structure.

---

## 15.1.5 Building the Widget Tree

The application view is constructed declaratively.

```rust
fn view(state: &CounterState) -> Node {
    Row {
        key: Some(Key::new("counter_root")),
        align: Align::Center,
        children: vec![
            Button {
                key: Some(Key::new("increment_button")),
                label: "Increment".into(),
                action: CounterAction::Increment,
                ..Default::default()
            }.into(),

            Spacer {
                width: 16,
                ..Default::default()
            }.into(),

            Text {
                key: Some(Key::new("counter_text")),
                text: format!("Count: {}", state.value),
                ..Default::default()
            }.into(),
        ],
        ..Default::default()
    }.into()
}
```

Characteristics:
- optional fields omitted via `Default`,
- keys are explicit and stable,
- state is read-only in the view,
- actions are referenced declaratively.

---

## 15.1.6 No Closures in the Tree

Note what is *not* present:

- no `on_click(|| ...)`,
- no captured state,
- no implicit lifetimes,
- no hidden side effects.

This is required for determinism and replay.

---

## 15.1.7 Semantics Are Mandatory

Interactable widgets attach semantics by default.

```rust
Button {
    semantics: Semantics {
        role: Role::Button,
        label: "Increment counter".into(),
        actions: vec![CounterAction::Increment],
    },
    ..Default::default()
}
```

Accessibility is not optional or inferred.

---

## 15.1.8 Lowering Boundary

The authoring layer ends here.

From this point:
- widgets desugar into Core IR ops,
- keys become stable node identities,
- actions become dispatchable descriptors,
- layout and paint are handled downstream.

No authoring concepts survive unchanged.

---

## 15.1.9 Why This Is Rust-Native

This approach embraces Rust idioms:

- structs and `Default`,
- explicit data ownership,
- enums for actions,
- no runtime reflection,
- compile-time guarantees.

The design is not a port of React or Flutter.

---

## 15.1.10 Summary

The authoring code demonstrates that:

- UI can be declared without closures,
- behavior is explicit and testable,
- ergonomics do not compromise determinism,
- the view is just data.

This is the foundation for a Rust-native, test-first UI framework.

---
