# 6.1 Semantics as a Mandatory Layer for Interactables

This section explains why semantics are mandatory for all interactive elements and how this requirement is enforced throughout the pipeline.
Making semantics non-optional is a core architectural decision that directly supports accessibility, determinism, and testability.

---

## 6.1.1 The Problem With Optional Semantics

In many UI frameworks, semantics and accessibility are treated as optional annotations.
This leads to several systemic issues:

- accessibility regressions are easy to introduce,
- semantics drift from actual behavior,
- tests cannot rely on semantic structure,
- tooling must guess intent heuristically.

Optional semantics inevitably become incomplete semantics.

---

## 6.1.2 Interactability Implies Meaning

An interactive element, by definition, has meaning.

If a user can:
- click,
- tap,
- focus,
- activate,

then the system must be able to answer:
- *what is this element?*
- *what can be done with it?*
- *what state is it in?*

Semantics are the mechanism by which this meaning is expressed.

---

## 6.1.3 Formal Definition of an Interactable

A node is considered **interactive** if it declares any of the following:

- a `HitRegion`,
- an `ActionSet`,
- focus participation.

This definition is structural and deterministic.

---

## 6.1.4 Mandatory Semantic Requirements

For any interactive node, the following semantics are required:

- a `SemanticRole`,
- at least one of `SemanticLabel` or `SemanticValue`,
- a declared `ActionSet` (even if empty but explicit).

These requirements are enforced during lowering.

---

## 6.1.5 Enforcement During Lowering

Lowering performs semantic validation:

- missing required semantics cause errors,
- invalid role/action combinations are rejected,
- incomplete semantic declarations are surfaced early.

Errors are:
- deterministic,
- reported with provenance,
- not deferred to runtime or platform adapters.

---

## 6.1.6 Benefits for Accessibility

Mandatory semantics ensure that:

- every interactive element is discoverable,
- screen readers have complete information,
- keyboard and assistive navigation is consistent,
- accessibility behavior matches interaction behavior.

Accessibility becomes a property of correctness, not compliance.

---

## 6.1.7 Benefits for Testing

Because semantics are guaranteed to exist:

- tests can select elements by role or label,
- interaction tests do not rely on pixel positions,
- behavior can be verified headlessly.

Example test queries:

```rust
find(role("button").label("Increment")).press();
assert_eq!(state.count, 1);
```

These tests are stable and non-flaky.

---

## 6.1.8 Benefits for Tooling and Automation

Mandatory semantics enable:

- automated UI inspection,
- semantic diffs instead of structural diffs,
- LLM-based reasoning about UI intent,
- validation tools that reason about behavior.

Tools no longer need to infer intent from visuals.

---

## 6.1.9 Relationship to Visual Appearance

Semantics are independent of visuals.

Rules:
- invisible elements may still have semantics,
- visual decoration does not imply semantics,
- semantics do not affect layout or paint.

This separation avoids accidental coupling.

---

## 6.1.10 Opting Out (and Why It Is Discouraged)

In rare cases, semantics may be explicitly suppressed.

Such cases must:
- declare an explicit suppression flag,
- provide a justification,
- be visible in debug output.

Opting out is discouraged and audited.

---

## 6.1.11 Comparison to Other Frameworks

Unlike frameworks where:
- semantics are inferred heuristically, or
- accessibility is layered post hoc,

this framework treats semantics as:
- required input,
- first-class Core IR data,
- validated and testable.

This difference is intentional.

---

## 6.1.12 Summary

Making semantics mandatory for interactables:

- enforces accessibility by construction,
- guarantees testable interaction surfaces,
- enables reliable tooling and automation,
- aligns meaning with behavior deterministically.

This requirement is foundational, not optional.

---
