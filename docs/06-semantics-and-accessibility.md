# 6. Semantics and Accessibility

This section defines the semantics and accessibility model of the framework.
Semantics are treated as a first-class, mandatory part of UI definition, not an optional add-on.

The goal is to ensure that *every interactive UI is accessible by construction* and fully testable in headless environments.

---

## 6.1 Semantics as a Core Concern

Semantics describe the *meaning* of UI elements, independent of their visual representation.

They enable:
- screen readers,
- keyboard navigation,
- automated testing,
- assistive technologies,
- non-visual interaction models.

In this framework, semantics are not inferred heuristically; they are declared explicitly.

---

## 6.2 Mandatory Semantics for Interactables

Any node that is interactive must declare semantics.

Specifically, if a node declares:
- a hit region, or
- an action set,

it must also declare:
- a semantic role,
- appropriate labels and/or values.

This rule is enforced during lowering and validation.

---

## 6.3 Semantic Roles

### `SemanticRole`

Semantic roles define what an element *is*.

Examples include:
- Button
- Text
- Image
- Checkbox
- Slider
- List
- ListItem

Roles are:
- platform-agnostic,
- stable across versions,
- used by accessibility adapters and tests.

Roles are never inferred implicitly.

---

## 6.4 Labels, Values, and State

### `SemanticLabel`
### `SemanticValue`
### `SemanticState`

These attributes describe:
- what the element is called,
- what value it represents,
- what state it is in (e.g. checked, disabled).

Properties:
- labels are localizable,
- values are structured data where possible,
- state changes are observable via Core IR updates.

---

## 6.5 Focus and Navigation Semantics

Focus behavior is explicit.

Semantics define:
- whether an element is focusable,
- focus traversal order,
- grouping behavior,
- default focus actions.

Keyboard and assistive navigation use the same semantic graph.

---

## 6.6 Action Semantics

Actions describe *what can be done* to an element.

Semantics define:
- the set of supported actions,
- default actions,
- action availability (enabled/disabled).

Actions are exposed to:
- accessibility systems,
- automated tests,
- alternative input methods.

---

## 6.7 Relationship Between Semantics and Layout

Semantics are independent of layout but reference layout geometry.

Examples:
- focus ring placement,
- screen reader spatial hints,
- hit target bounds.

Semantics never affect layout decisions.

---

## 6.8 Semantics Tree

The semantics tree is derived from Core IR.

Properties:
- deterministic structure,
- explicit ordering,
- stable identity references.

The semantics tree may differ from the visual tree to improve accessibility, but this transformation is explicit and inspectable.

---

## 6.9 Headless Semantics and Testing

Semantics are fully available in headless mode.

This enables tests such as:
- querying nodes by role or label,
- asserting available actions,
- simulating accessibility interactions,
- verifying traversal order.

No rendering is required to test accessibility behavior.

---

## 6.10 Platform Adapters

Platform-specific accessibility APIs are implemented via adapters.

Adapters:
- consume the semantics tree,
- translate roles and attributes,
- do not reinterpret semantics.

Platform quirks are handled at the adapter layer, not in Core IR.

---

## 6.11 Validation and Error Reporting

Semantic validation includes:
- missing required semantics,
- invalid role/action combinations,
- inconsistent labels or states.

Errors are reported deterministically with provenance information.

---

## 6.12 Summary

The semantics and accessibility model:

- makes accessibility mandatory by construction,
- separates meaning from presentation,
- enables deterministic testing and tooling,
- supports all platforms via adapters.

Accessibility is not a feature; it is a property of the system.

---
