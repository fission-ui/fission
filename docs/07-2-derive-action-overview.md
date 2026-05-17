# 7.2 `#[fission_reducer]` and `#[fission_action]` Overview

This section describes the action macro system: `#[fission_reducer]` for compact one-reducer actions, and `#[fission_action]` for manually declared action types that need reuse or public documentation.

---

## 7.2.1 Purpose of the action macros

The macros exist to:

- avoid stringly-typed action identifiers,
- generate stable, globally unique action IDs,
- define payload schemas declaratively,
- integrate actions with semantics, reducers, and tests.

It provides ergonomics without leaking closures into the system.

---

## 7.2.2 Action Definition Model

An action is defined as a Rust type.

Recommended compact form:

```rust
#[fission_reducer(Increment)]
fn increment(state: &mut CounterState) {
    state.count += 1;
}

#[fission_reducer(SetCount)]
fn set_count(state: &mut CounterState, value: i32) {
    state.count = value;
}
```

Manual form for shared actions:

```rust
#[fission_action]
pub struct ResetCounter;
```

The generated or manual action type remains the single source of truth for dispatch identity.

---

## 7.2.3 Generated Artifacts

The action macros generate:

- a stable `ActionId`,
- serialization and deserialization logic,
- payload schema metadata,
- validation hooks,
- semantic registration data.

All generated code is deterministic and explicit.

---

## 7.2.4 Stable Action Identity

Action identity is derived from:

- crate identity,
- type name,
- optional explicit version tag.

This ensures:
- no accidental collisions,
- stability across builds,
- compatibility with snapshots and replay.

Action IDs are opaque and strongly typed at runtime.

---

## 7.2.5 Payload Schema Generation

For actions with fields:

- field names and types form the payload schema,
- schemas are validated at dispatch time,
- missing or invalid payloads are rejected deterministically.

Schemas are part of the Core action metadata.

---

## 7.2.6 Integration With Semantics

Actions defined via `#[fission_reducer]` or `#[fission_action]` can be:

- attached to semantic roles,
- exposed to accessibility systems,
- discovered by tests and tooling.

The derive macro ensures actions are compatible with semantic validation rules.

---

## 7.2.7 Reducer Integration

Reducers pattern-match on action types:

```rust
fn reduce(state: &mut State, action: Action, target: NodeId) {
    if let Some(Increment) = action.downcast() {
        state.count += 1;
    }
}
```

This is:
- type-safe,
- explicit,
- exhaustive where desired.

---

## 7.2.8 Testing and Replay Support

Because actions are data:

- tests may construct actions directly,
- action sequences can be recorded and replayed,
- payloads are inspectable and comparable.

The derive macro guarantees serializability.

---

## 7.2.9 Versioning and Evolution

Action definitions may evolve.

Rules:
- adding optional fields is backward-compatible,
- removing fields is breaking,
- semantic meaning must remain stable per version.

Version tags may be attached explicitly if needed.

---

## 7.2.10 Debugging and Instrumentation

Derived actions include debug metadata:

- human-readable names,
- payload formatting helpers,
- provenance attachment points.

This metadata is non-semantic and optional.

---

## 7.2.11 Constraints and Limitations

Actions must obey constraints:

- payloads must be serializable,
- no references or lifetimes,
- no closures or function pointers.

These constraints are enforced at compile time.

---

## 7.2.12 Summary

`#[fission_reducer]` and `#[fission_action]` provide:

- ergonomic, type-safe action and reducer definitions,
- stable and inspectable action identities,
- seamless integration with semantics and reducers,
- strong guarantees for testing and replay.

They are the bridge between author ergonomics and Core correctness.

---
