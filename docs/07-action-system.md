# 7. Action System

This section defines the Action System: how user intent, accessibility interaction, and programmatic events are represented, validated, and executed.
Actions are the sole mechanism by which interaction causes state change.

There are no implicit callbacks or hidden side effects.

---

## 7.1 Actions as First-Class Data

An **Action** represents an intent to change state.

Actions are:
- explicit data structures,
- declarative and inspectable,
- serializable and replayable,
- deterministic.

Actions are not closures and do not capture environment.

---

## 7.2 Why an Action System (Not Callbacks)

Callback-based systems suffer from:

- hidden control flow,
- nondeterministic execution order,
- poor testability,
- tight coupling to runtime context.

The Action System replaces callbacks with:
- explicit intent,
- centralized routing,
- deterministic reducers.

This mirrors modern state-management systems while remaining UI-native.

---

## 7.3 Action Declaration

Actions are declared as part of semantics.

Each action declaration includes:
- a stable action identifier,
- an optional payload schema,
- enabled/disabled state,
- semantic meaning.

Example (conceptual):

```rust
Action {
    id: ActionId::Activate,
    payload: None,
    enabled: true,
}
```

Action identifiers are stable and versioned.

---

## 7.4 Action Identifiers

Action identifiers are:

- strongly typed,
- globally stable,
- not stringly-typed at runtime.

Identifiers may be:
- built-in (e.g. Activate, Increment),
- user-defined via derive macros (e.g. `#[derive(Action)]`).

Derivation produces:
- a stable identifier,
- serialization metadata,
- validation hooks.

---

## 7.5 Action Payloads

Actions may carry payloads.

Payloads are:
- structured and typed,
- validated during dispatch,
- serializable.

Examples:
- text input changes,
- slider value updates,
- selection indices.

Payload schemas are part of the action definition.

---

## 7.6 Action Routing

Actions are routed deterministically:

1. Action is invoked via semantics or input.
2. Target node identity is resolved.
3. Action is dispatched to the reducer.
4. State update occurs.
5. UI is rebuilt deterministically.

There is no bubbling or capture unless explicitly modeled.

---

## 7.7 Reducers

Reducers are pure functions:

```rust
fn reduce(state: &mut AppState, action: Action, target: NodeId)
```

Reducers:
- are deterministic,
- perform explicit state transitions,
- have no side effects beyond state mutation.

Reducers do not access UI structure directly.

---

## 7.8 Validation and Enforcement

The system validates that:

- only declared actions may be invoked,
- payloads match schemas,
- disabled actions cannot be executed,
- role/action combinations are valid.

Violations are deterministic errors.

---

## 7.9 Accessibility and Actions

Accessibility systems invoke the same actions.

There is no separate accessibility execution path.

This guarantees:
- identical behavior across input modalities,
- testability of accessibility interaction,
- semantic correctness.

---

## 7.10 Testing Actions

Actions are easily testable.

Examples:

```rust
find(role("button").label("Increment"))
    .action("activate")
    .invoke();

assert_eq!(state.count, 1);
```

Tests may also:
- dispatch actions directly,
- replay recorded action sequences,
- assert state transitions.

---

## 7.11 Instrumentation and Replay

Actions can be:
- logged,
- recorded,
- replayed deterministically.

This enables:
- time-travel debugging,
- regression reproduction,
- LLM-assisted analysis.

Action logs are platform-independent.

---

## 7.12 Error Handling

Action-related errors include:

- unknown action identifiers,
- invalid payloads,
- dispatch to non-interactive nodes.

Errors are:
- deterministic,
- reported with provenance,
- never silently ignored.

---

## 7.13 Summary

The Action System:

- replaces callbacks with explicit intent,
- unifies user, accessibility, and test interaction,
- guarantees deterministic state changes,
- enables powerful testing and tooling.

It is the backbone of interaction in the framework.

---
