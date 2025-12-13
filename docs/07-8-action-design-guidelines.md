# 7.8 Action Design Guidelines

This section provides concrete guidelines for designing actions that remain deterministic, testable, accessible, and evolvable.
These guidelines are normative: violating them undermines the guarantees of the framework.

Actions define *what can happen* in an application. Poorly designed actions create long-term technical debt.

---

## 7.8.1 Prefer Small, Intent-Focused Actions

Actions should represent a single, clear intent.

Good examples:
- `Increment`
- `SubmitForm`
- `SelectItem { id }`

Poor examples:
- `HandleButtonClick`
- `UpdateEverything`
- `DoStuff`

If an action requires a long explanation, it is probably too broad.

---

## 7.8.2 Actions Describe Intent, Not Mechanism

Actions must describe *what the user wants*, not *how the system should do it*.

Avoid:
- UI-specific terms (e.g. “clicked”, “tapped”),
- layout-specific language,
- implementation details.

Prefer:
- semantic intent (`Activate`, `Select`, `Confirm`),
- domain-level meaning.

This keeps actions input-agnostic and accessible.

---

## 7.8.3 Keep Payloads Minimal and Structured

Payloads should:
- include only data required to express intent,
- avoid derived or redundant data,
- use stable identifiers instead of indices.

Good:
```rust
SelectItem { item_id: ItemId }
```

Bad:
```rust
SelectItem { index: usize, x: f32, y: f32 }
```

Minimal payloads improve compatibility and replay.

---

## 7.8.4 Avoid Encoding State Transitions in Actions

Actions must not encode *how state changes*.

Avoid:
- `SetCounterToFive`
- `IncrementUnlessMax`

Instead:
- encode intent (`Increment`),
- let reducers enforce rules.

Reducers own state transitions.

---

## 7.8.5 One Reducer Responsibility per Action

Each action should have a clear ownership boundary.

Rules:
- one primary reducer owns the action,
- secondary effects must be explicit,
- fan-out requires explicit composition.

This avoids hidden coupling and ordering bugs.

---

## 7.8.6 Design for Accessibility First

If an action can be triggered by a user, it must be:

- discoverable via semantics,
- invokable by accessibility systems,
- meaningful without visuals.

Ask:
“Would this action make sense to a screen reader user?”

If not, redesign it.

---

## 7.8.7 Ensure Actions Are Test-Friendly

A good action:
- can be constructed directly in tests,
- can be invoked without rendering,
- produces observable state changes.

If testing an action requires pixel inspection, the design is wrong.

---

## 7.8.8 Version Actions Deliberately

When evolving actions:

- add optional fields instead of required ones,
- preserve semantic meaning,
- use aliases when renaming or moving actions.

Breaking action semantics breaks replay and tests.

---

## 7.8.9 Avoid Overloading Actions

Do not reuse a single action for unrelated intents.

Avoid:
- `Update`
- `Change`
- `Modify`

Explicit actions lead to clearer reducers, traces, and tests.

---

## 7.8.10 Treat Actions as Public API

Actions form a public contract between:
- UI,
- state management,
- tests,
- tooling,
- accessibility systems.

Design them as you would a public API:
- stable,
- well-named,
- documented.

---

## 7.8.11 Review Checklist

Before adding an action, ask:

- Is the intent clear and singular?
- Is the payload minimal and stable?
- Is it accessible and semantic?
- Can it be replayed deterministically?
- Will it still make sense in two years?

If any answer is “no”, redesign.

---

## 7.8.12 Summary

Well-designed actions:

- encode intent, not mechanics,
- remain stable and evolvable,
- support accessibility and testing,
- enable deterministic replay.

Action quality directly determines framework quality.

---
