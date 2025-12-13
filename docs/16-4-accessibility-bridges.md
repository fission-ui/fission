# 16.4 Accessibility Bridges

This section describes how **accessibility bridges** connect the Core Runtime’s semantics model to
platform-specific accessibility APIs.
Accessibility is treated as a first-class, deterministic subsystem—not an afterthought or best-effort mapping.

The Core defines meaning. Platforms translate it.

---

## 16.4.1 Accessibility as a Core Responsibility

Accessibility is owned by the Core Runtime.

Core responsibilities:
- define semantic roles, labels, values, and actions,
- define focus order and traversal,
- define enabled/disabled and visibility state,
- define action semantics (activate, increment, scroll, etc.).

Platforms are not allowed to infer accessibility meaning.

---

## 16.4.2 Role of Accessibility Bridges

Accessibility bridges:

- translate Core semantics to native platform APIs,
- forward native accessibility actions back to the Core,
- preserve deterministic ordering and identity,
- expose accessibility trees for inspection and testing.

Bridges are adapters, not decision-makers.

---

## 16.4.3 Core Semantics Recap

Each accessible node in Core provides:

- stable node identity,
- semantic role (e.g. Button, Text, Slider),
- human-readable label,
- optional value and state,
- supported actions,
- focus and traversal metadata.

This data is complete and sufficient.

---

## 16.4.4 Platform Mappings

### 16.4.4.1 Desktop Platforms

- **Windows** → UI Automation
- **macOS** → Accessibility (AX) API
- **Linux** → AT-SPI

Mappings are explicit:
- Core roles map to native roles,
- Core actions map to native invoke patterns,
- native state queries reflect Core snapshot state.

---

### 16.4.4.2 Mobile Platforms

- **iOS** → UIKit Accessibility
- **Android** → AccessibilityNodeInfo / Services

Rules:
- Core focus order drives native traversal,
- native gestures dispatch Core actions,
- screen readers activate Core actions directly.

---

### 16.4.4.3 Web Platforms

- **Web** → ARIA roles and attributes

Rules:
- DOM nodes are minimal and semantics-driven,
- ARIA mirrors Core semantics exactly,
- browser accessibility actions dispatch Core actions.

The DOM is not the source of truth.

---

## 16.4.5 Focus Management

Focus is Core-controlled.

Rules:
- Core defines focusable nodes,
- Core defines traversal order,
- platform focus events update Core state,
- focus changes produce deterministic snapshots.

Platforms must not reorder focus implicitly.

---

## 16.4.6 Action Dispatch from Accessibility

Accessibility activation behaves identically to pointer input.

Flow:
1. Native accessibility action occurs.
2. Bridge maps it to a Core action.
3. Core dispatches the action to reducers.
4. UI updates and snapshots are produced.

There is a single interaction path.

---

## 16.4.7 Accessibility Tree Inspection

Accessibility bridges expose the accessibility tree for tooling.

Capabilities:
- inspect roles and labels,
- inspect focus order,
- simulate accessibility actions,
- diff accessibility snapshots.

This is critical for testing and compliance.

---

## 16.4.8 Determinism Guarantees

Accessibility behavior is deterministic because:

- semantics are explicit data,
- traversal order is canonical,
- actions are descriptors,
- platform APIs are treated as IO.

Identical snapshots yield identical accessibility trees.

---

## 16.4.9 Error Handling and Gaps

When a platform lacks a direct mapping:

- the bridge reports unsupported features explicitly,
- Core semantics remain intact,
- fallbacks are deterministic and documented.

Silent degradation is forbidden.

---

## 16.4.10 Testing Accessibility

Accessibility is testable headlessly.

Tests can:
- assert semantic roles and labels,
- assert focus traversal order,
- inject accessibility actions,
- verify resulting Core actions and snapshots.

No real screen reader is required for correctness testing.

---

## 16.4.11 Performance Considerations

Accessibility bridges are incremental:

- trees update only on semantic changes,
- identity stability prevents full rebuilds,
- platform APIs are used efficiently.

Performance optimizations do not affect semantics.

---

## 16.4.12 Why This Design Matters

This design ensures that:

- accessibility parity is guaranteed,
- accessibility regressions are detectable,
- platforms cannot “accidentally” break accessibility,
- LLMs and CI can reason about accessibility meaningfully.

Accessibility is engineered, not retrofitted.

---

## 16.4.13 Summary

Accessibility bridges succeed because:

- the Core defines all meaning,
- platforms only translate,
- interaction paths are unified,
- testing and inspection are first-class.

An accessible UI is not a variant—it is the UI.

---
