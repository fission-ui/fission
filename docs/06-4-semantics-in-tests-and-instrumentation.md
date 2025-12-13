# 6.4 Semantics in Tests and Instrumentation

This section describes how semantics form the primary interface for testing, verification, and instrumentation.
By making semantics explicit, deterministic, and mandatory, the framework enables tests to reason about *meaning* rather than *pixels*.

Semantics are the preferred test surface.

---

## 6.4.1 Semantics as the Testing API

Tests interact with the UI primarily through semantics.

This enables tests to:
- find elements by role, label, or value,
- query available actions,
- assert state and accessibility properties,
- simulate interactions deterministically.

Visual appearance is not required for correctness testing.

---

## 6.4.2 Semantic Selectors

The test framework exposes semantic selectors.

Examples:

```rust
find(role("button").label("Increment"))
find(role("slider").value(75))
find(role("list").child(role("list_item")))
```

Selectors are:
- composable,
- stable across layout changes,
- independent of rendering backend.

---

## 6.4.3 Action Simulation

Actions are invoked via semantics.

Example:

```rust
find(role("button").label("Increment"))
    .action("activate")
    .invoke();
```

Invocation:
- emits the declared Core action,
- passes through the reducer pipeline,
- triggers a deterministic rebuild.

There are no synthetic callbacks or event mocking.

---

## 6.4.4 Assertions on Semantic State

Tests may assert:

- presence or absence of roles,
- label and value contents,
- enabled/disabled action state,
- focusability and traversal order.

Example:

```rust
find(role("button"))
    .assert_enabled("activate");
```

Assertions operate on Core snapshots, not runtime side effects.

---

## 6.4.5 Semantics vs Layout Assertions

Semantics and layout serve different testing purposes.

- Semantics tests verify *behavior and meaning*.
- Layout tests verify *geometry and structure*.

Both are supported, but semantics tests are preferred where possible because they are more robust.

---

## 6.4.6 Headless Semantics Testing

Semantics are fully available in headless mode.

This allows:
- fast CI execution,
- no GPU or window system requirements,
- deterministic test runs.

Headless semantics tests are identical to rendered semantics tests.

---

## 6.4.7 Instrumentation Hooks

Instrumentation may observe semantics at multiple stages:

- post-lowering,
- post-canonicalization,
- post-layout.

Instrumentation can:
- capture semantic snapshots,
- trace semantic changes across frames,
- correlate semantics with provenance.

Instrumentation is optional and zero-cost when disabled.

---

## 6.4.8 Semantic Snapshots and Diffs

Semantic snapshots capture:

- roles and hierarchy,
- labels and values,
- action sets and states,
- focus and traversal metadata.

Diffing semantic snapshots highlights:
- meaningful behavior changes,
- accessibility regressions,
- unintended interaction changes.

Visual noise is eliminated.

---

## 6.4.9 Accessibility Regression Testing

Semantics enable automated accessibility tests such as:

- ensuring all interactables have labels,
- verifying correct roles are used,
- checking traversal order consistency.

These tests run as part of standard CI.

---

## 6.4.10 LLM and Tooling Integration

Because semantics are explicit and structured:

- LLMs can reason about UI intent,
- tools can suggest fixes or improvements,
- automated refactors can preserve behavior.

Semantics provide a machine-readable contract.

---

## 6.4.11 Failure Diagnostics

When tests fail, semantics provide:

- clear descriptions of missing or incorrect elements,
- provenance-backed explanations,
- minimal, actionable diffs.

This reduces debugging time significantly.

---

## 6.4.12 Summary

Using semantics as the primary test and instrumentation surface:

- improves test robustness,
- enables headless verification,
- supports accessibility guarantees,
- empowers advanced tooling and automation.

Semantics turn UI testing from pixel comparison into behavioral verification.

---
