# 7.5 Namespaces, Aliases, and Backwards Compatibility

This section defines how actions are namespaced, how aliases are used to preserve compatibility, and how action evolution is handled over time without breaking tests, replays, or accessibility integrations.

The goal is to allow actions to evolve while keeping identity stable and behavior explainable.

---

## 7.5.1 Namespaces as Identity Structure

Actions live in explicit namespaces derived from their defining crate and module path.

Namespaces:
- prevent accidental collisions,
- enable open-world extension,
- provide human-meaningful grouping.

Example (conceptual):

```text
app::counter::actions::Increment
```

Namespaces are part of the canonical identity but are not exposed as strings in user code.

---

## 7.5.2 Why Namespaces Matter

Without namespaces:
- independent libraries collide,
- plugins cannot coexist safely,
- action identity becomes centralized and brittle.

Namespaces allow:
- decentralized action definition,
- predictable identity derivation,
- long-term ecosystem growth.

---

## 7.5.3 Action Aliases

### Definition

An **action alias** declares that a new action identity is semantically equivalent to an old one.

Aliases are explicit, versioned mappings.

Example (conceptual):

```rust
#[fission_action]
#[action(alias = "app::counter::Increment@v1")]
pub struct IncrementV2;
```

Aliases are part of action metadata.

---

## 7.5.4 Use Cases for Aliases

Aliases are used to:

- rename actions,
- move actions between modules or crates,
- evolve payload schemas compatibly,
- preserve snapshot and replay compatibility.

Aliases avoid breaking changes while allowing refactors.

---

## 7.5.5 Alias Resolution Rules

During dispatch and replay:

- primary identity is preferred,
- aliases are resolved deterministically,
- ambiguous alias resolution is an error.

Alias resolution is explicit and observable.

---

## 7.5.6 Backwards Compatibility Guarantees

The framework guarantees that:

- actions with registered aliases remain invokable,
- old action logs can be replayed,
- tests referencing old identities continue to work.

Breaking compatibility requires explicit opt-in.

---

## 7.5.7 Payload Compatibility and Aliases

When aliasing actions with payload changes:

Rules:
- removed fields must have defaults,
- added required fields are forbidden,
- transformations must be deterministic.

Optional migration functions may be provided, but must be pure.

---

## 7.5.8 Deprecation Strategy

Actions may be deprecated without removal.

Deprecation:
- emits warnings in debug and tooling,
- does not affect runtime behavior,
- is visible in metadata.

Removal is only allowed in major version changes.

---

## 7.5.9 Interaction With Semantics and Accessibility

Aliases preserve semantic meaning.

Accessibility systems:
- continue to expose the same actions,
- do not need to distinguish aliases,
- remain compatible with old assistive tooling.

Semantics are stable across aliases.

---

## 7.5.10 Testing and Tooling Implications

Aliases ensure:

- long-lived tests remain valid,
- recorded interaction traces replay correctly,
- diffs remain meaningful across refactors.

Tooling can surface alias usage for auditability.

---

## 7.5.11 Error Handling

Errors include:
- conflicting aliases,
- incompatible payload evolution,
- ambiguous resolution chains.

All errors are:
- deterministic,
- detected early,
- reported with provenance.

---

## 7.5.12 Summary

Namespaces, aliases, and compatibility rules:

- enable safe action evolution,
- preserve determinism and replayability,
- support large-scale refactoring,
- keep accessibility and tests stable over time.

They ensure the action system can grow without breaking its guarantees.

---
