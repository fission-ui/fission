# 6.5 Focus Order and Accessibility Traversal

This section defines how focus order and accessibility traversal are modeled, validated, and tested.
Traversal is explicit, deterministic, and derived from Core semantics rather than visual heuristics.

Focus and traversal are semantic properties, not side effects of layout or rendering.

---

## 6.5.1 Goals of Explicit Traversal

Explicit traversal exists to:

- guarantee predictable keyboard navigation,
- ensure screen readers follow meaningful order,
- avoid platform-specific heuristics,
- make traversal testable and debuggable.

Traversal must be stable across rebuilds and platforms.

---

## 6.5.2 Focusability Semantics

Focusability is an explicit semantic attribute.

Each node declares:
- focusable / not focusable,
- optional default focus action,
- participation in traversal.

Non-focusable nodes may still contribute labels or grouping.

---

## 6.5.3 Traversal Graph vs Visual Tree

Traversal order is derived from a **semantics graph**, not the visual tree.

Reasons:
- visual order may not match logical order,
- overlays and decorations should not affect traversal,
- accessibility grouping may differ from layout grouping.

The semantics graph is explicit and inspectable.

---

## 6.5.4 Default Traversal Order

By default, traversal follows:

1. Semantics tree order (pre-order),
2. Explicit child ordering,
3. Stable identity ordering where required.

Defaults are deterministic and documented.

---

## 6.5.5 Explicit Traversal Overrides

Authors may override traversal using explicit semantics:

- traversal groups,
- ordering hints,
- skip flags.

Overrides:
- are explicit data,
- are validated for consistency,
- never rely on layout geometry.

---

## 6.5.6 Grouping and Scope Effects

Semantic grouping affects traversal:

- groups may define entry/exit points,
- scopes isolate traversal subgraphs,
- embedded content may define its own traversal root.

Grouping rules are explicit and deterministic.

---

## 6.5.7 Interaction With Focus State

Focus state is observable and deterministic.

Rules:
- at most one focused node per focus scope,
- focus transitions emit observable events,
- focus changes are driven by actions.

Focus never changes implicitly.

---

## 6.5.8 Platform Adapter Responsibilities

Platform adapters must:

- preserve Core-defined traversal order,
- map traversal semantics to platform APIs,
- document any unavoidable deviations.

Adapters must not invent traversal heuristics.

---

## 6.5.9 Testing Focus and Traversal

Traversal is fully testable in headless mode.

Example:

```rust
assert_traversal_order(vec![
    role("button").label("Increment"),
    role("text").label("Count"),
]);
```

Tests may also simulate:
- tab navigation,
- reverse traversal,
- focus jumps.

---

## 6.5.10 Accessibility-Specific Traversal

Accessibility traversal may include:

- non-focusable descriptive nodes,
- value announcements,
- grouping announcements.

These behaviors are driven by semantics, not adapters.

---

## 6.5.11 Validation and Error Handling

Lowering validates traversal semantics:

- cycles are forbidden,
- invalid overrides are rejected,
- ambiguous ordering is an error.

Failures are deterministic and reported with provenance.

---

## 6.5.12 Summary

Explicit focus order and accessibility traversal:

- ensure predictable navigation,
- decouple semantics from visuals,
- enable deterministic testing,
- provide consistent cross-platform behavior.

Traversal is a semantic contract, not an implementation detail.

---
