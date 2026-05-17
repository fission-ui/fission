# 15. Example: Counter Application End-to-End

This section presents a complete **end-to-end example** of the framework using a simple Counter application.
The goal is not the counter itself, but to demonstrate how authoring, Core IR, actions, layout, rendering, and testing fit together coherently and deterministically.

This example is intentionally minimal and fully inspectable.

---

## 15.1 Problem Statement

We want an application that:

- displays a counter value,
- increments the counter when a button is activated,
- is fully testable without rendering,
- produces deterministic layout and behavior.

This mirrors the canonical Flutter counter example, but under this framework’s constraints.

---

## 15.2 Application State and Actions

### 15.2.1 State Definition

```rust
struct CounterState {
    value: i32,
}
```

State is explicit, serializable, and reducer-owned.

---

### 15.2.2 Action Definition

```rust
#[fission_action]
enum CounterAction {
    Increment,
}
```

Actions are descriptors, not closures.
They are stable, hashable, and replayable.

---

### 15.2.3 Reducer

```rust
fn reducer(state: &mut CounterState, action: CounterAction) {
    match action {
        CounterAction::Increment => {
            state.value += 1;
        }
    }
}
```

Reducers are pure and deterministic.

---

## 15.3 Authoring Layer (Struct-Based Widgets)

```rust
CounterView {
    key: Some("counter_root"),
    children: vec![
        Button {
            key: Some("increment_button"),
            label: "Increment",
            on_activate: CounterAction::Increment,
        },
        Text {
            key: Some("counter_text"),
            value: bind!(state.value),
        }
    ]
}
```

Notes:
- widgets are structs with defaults,
- no closures exist in the tree,
- actions are referenced declaratively,
- state is bound explicitly.

---

## 15.4 Lowering to Core IR (Conceptual)

The authoring tree lowers to Core IR ops such as:

- `Frame`
- `SetSemantics(Role::Button)`
- `BindAction(Increment)`
- `TextLayout`
- `SubscribeState(value)`

No widget-specific concepts survive lowering.

---

## 15.5 Layout and Geometry

After layout, the snapshot contains:

```text
counter_root
├─ increment_button  rect=(x=100,y=50,w=88,h=32)
└─ counter_text      rect=(x=100,y=100,w=64,h=20)
```

Geometry is deterministic and queryable.

---

## 15.6 Interaction Flow

1. User activates the button (pointer or accessibility).
2. Normalized event is routed via hit testing.
3. `CounterAction::Increment` is dispatched.
4. Reducer updates state.
5. Layout and paint are invalidated.
6. A new snapshot is produced.

Each step is observable.

---

## 15.7 Test: Visual Structure and Layout

```rust
let app = TestHarness::new(CounterApp::default());

let button = find("increment_button");
assert_eq!(button.rect().width, 88);

let text = find("counter_text");
assert_eq!(text.text(), "0");
```

No rendering is required.

---

## 15.8 Test: Interaction and State Change

```rust
click("increment_button");

assert_action_dispatched(CounterAction::Increment);
assert_eq!(find("counter_text").text(), "1");
```

Interaction is validated through actions and snapshots.

---

## 15.9 Test: Accessibility Equivalence

```rust
accessibility_activate("increment_button");
assert_eq!(find("counter_text").text(), "2");
```

Pointer and accessibility paths are identical.

---

## 15.10 Structural Snapshot Diff

Before click:

```text
Text(value=0)
```

After click:

```text
Text(value=1)
```

The diff explains the change directly.

---

## 15.11 Optional Pixel Test

```rust
assert_pixels_match("counter_initial.png");
```

Pixel tests are optional and scoped.

---

## 15.12 Determinism Guarantees Demonstrated

This example demonstrates that:

- state changes are explicit,
- actions are traceable,
- layout is deterministic,
- tests do not depend on time or rendering,
- failures are explainable.

The same test behaves identically everywhere.

---

## 15.13 Scaling the Pattern

The same pattern scales to:

- complex layouts,
- animations,
- scrolling,
- media,
- 3D embeds.

No new testing concepts are introduced.

---

## 15.14 Summary

This counter example shows the framework end to end:

- authoring → Core IR → layout → snapshot,
- interaction → action → reducer → snapshot,
- tests assert behavior, not implementation.

If a UI can be built this way, it can be tested, reasoned about, and trusted.

---
