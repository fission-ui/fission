# Action System (Revised and Canonical)

This document defines the **canonical Action System** for the framework.
It supersedes earlier descriptions and should be treated as the source of truth.

The action system is designed to be:
- deterministic,
- closure-free in the widget tree,
- fully serializable and replayable,
- ergonomic for humans and LLMs,
- modular at scale (no monolithic reducers).

---

## 1. Core Principles

1. **Actions are data, not behavior**
   - Widgets never store closures or function pointers.
   - Widgets store *descriptors* of intent only.

2. **Typed at authoring time, erased at runtime**
   - Authors work with strongly typed Rust actions.
   - The Core Runtime works with erased, canonical representations.

3. **Handlers are registered, not embedded**
   - Action handlers are registered during app construction.
   - Registration is deterministic and owned by the runtime.

4. **Dispatch is synchronous and deterministic**
   - No async pub/sub.
   - No background delivery.
   - No unordered fanout.

---

## 2. Action Types (`#[derive(Action)]`)

An action is defined as a pure data type.

```rust
#[derive(Action, Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct Increment;
```

The `#[derive(Action)]` macro generates:

- a stable `ActionId`
- canonical encoding/decoding logic
- optional debug metadata (name, version)
- compile-time linkage between type and identity

Actions:
- must be deterministic
- must not capture environment
- must be serializable via a canonical format

---

## 3. ActionEnvelope (Runtime Representation)

All actions lower to a single, erased representation.

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActionEnvelope {
    pub id: ActionId,
    #[serde(with = "serde_bytes")]
    pub payload: Vec<u8>,
    pub target: Option<WidgetNodeId>,
}
```

Properties:
- fully serializable
- comparable and hashable
- suitable for logging, replay, diffing
- independent of Rust type system at runtime

The widget tree stores **only** `ActionEnvelope` (or a thin alias such as `ActionBinding`).

---

## 4. Build-Time Binding (Ergonomic API)

Handlers are associated with actions during app construction via a **Build Context**.

```rust
pub type Handler<S, A> = fn(&mut S, A);

pub struct BuildCtx<S> {
    registry: ActionRegistry<S>,
}
```

Binding an action:

```rust
fn on_increment(state: &mut CounterState, _: Increment) {
    state.value += 1;
}

Button {
    on_press: Some(ctx.bind::<Increment>(on_increment)),
    ..Default::default()
}
```

What `bind` does:

1. Registers the handler under `Increment::ID` in the registry.
2. Returns a pure `ActionEnvelope` with:
   - `id = Increment::ID`
   - `payload = Increment::encode(&Increment)`
   - `target = None` (or filled automatically by the runtime)

No side effects escape the build context.

---

## 5. Handler Registration and Registry

Internally, the runtime owns a deterministic registry:

```rust
struct ActionRegistry<S> {
    handlers: BTreeMap<ActionId, Box<dyn Fn(&mut S, &ActionEnvelope)>>,
}
```

Registration is:
- deterministic
- ordered
- complete before runtime execution begins

Multiple handlers per `ActionId` are discouraged but allowed if ordering is defined.

---

## 6. Dispatch Model

When an action occurs (pointer, keyboard, accessibility):

1. Core emits an `ActionEnvelope`.
2. Envelope is synchronously dispatched.
3. Matching handler(s) are invoked.
4. State is mutated.
5. A new snapshot is produced.

There is no background queue unless explicitly modeled and drained deterministically.

---

## 7. Targeted Actions (Widget-Scoped Behavior)

Actions may optionally include a target:

```rust
ActionEnvelope {
    id,
    payload,
    target: Some(widget_id),
}
```

This enables:
- widget-local state
- instance-specific reducers
- scalable UIs without global state explosion

Handlers may use `target` to select the correct substate.

---

## 8. Determinism Guarantees

The action system is deterministic because:

- `ActionId` is stable
- payload encoding is canonical
- handler registration is deterministic
- dispatch order is explicit
- time is Core-owned
- no hidden side effects exist

Given:
- the same initial state,
- the same action trace,
- the same Core version,

the result is guaranteed to be identical.

---

## 9. Action Tracing and Replay

Because actions are envelopes:

- they can be recorded verbatim,
- replayed without rebuilding widgets,
- diffed across runs,
- inspected by tools and LLMs.

Replay requires only:
- initial snapshot,
- action envelope sequence,
- deterministic registry construction.

---

## 10. What Is Explicitly Forbidden

The following are not allowed:

- closures in widget structs
- `Box<dyn Action>` stored in trees
- implicit global registries
- async or unordered dispatch
- handler registration outside build context
- state mutation outside handlers

These rules are enforced to preserve long-term correctness.

---

## 11. Mental Model Summary

- Widgets **describe intent**
- Actions **describe events**
- Handlers **own behavior**
- The Core **owns time and order**
- Everything important is **data**

This separation is what enables testing, replay, tooling, and LLM-native workflows at scale.
