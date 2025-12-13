# 7.2 `#[derive(Action)]` Overview

This section describes the `#[derive(Action)]` system: how actions are defined, identified, validated, and integrated into the Core and authoring layers.
The derive macro is the primary author-facing mechanism for defining custom actions without sacrificing determinism or introspectability.

---

## 7.2.1 Purpose of `#[derive(Action)]`

The derive macro exists to:

- avoid stringly-typed action identifiers,
- generate stable, globally unique action IDs,
- define payload schemas declaratively,
- integrate actions with semantics, reducers, and tests.

It provides ergonomics without leaking closures into the system.

---

## 7.2.2 Action Definition Model

An action is defined as a Rust type.

Example:

```rust
#[derive(Action)]
pub struct Increment;
```

More complex actions may include payload data:

```rust
#[derive(Action)]
pub struct SetCount {
    pub value: i32,
}
```

The type definition is the single source of truth.

---

## 7.2.3 Generated Artifacts

Deriving `Action` generates:

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

Actions defined via `#[derive(Action)]` can be:

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

`#[derive(Action)]` provides:

- ergonomic, type-safe action definitions,
- stable and inspectable action identities,
- seamless integration with semantics and reducers,
- strong guarantees for testing and replay.

It is the bridge between author ergonomics and Core correctness.

---
