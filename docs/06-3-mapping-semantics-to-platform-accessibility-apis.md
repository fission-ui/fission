# 6.3 Mapping Semantics to Platform Accessibility APIs

This section explains how Core semantics are mapped to platform-specific accessibility APIs.
The key principle is **adaptation, not interpretation**: platform adapters translate Core semantics faithfully without inventing or inferring meaning.

All semantic meaning originates in Core IR.

---

## 6.3.1 Adapter-Based Architecture

Accessibility integration is implemented via platform adapters.

Each adapter:
- consumes the semantics tree derived from Core IR,
- maps Core roles, labels, values, and actions to platform APIs,
- handles platform quirks and limitations locally.

Adapters never modify Core semantics.

---

## 6.3.2 Separation of Concerns

Responsibilities are strictly divided:

- **Core IR:** defines semantic meaning
- **Semantics Tree:** canonical, platform-agnostic representation
- **Platform Adapter:** translation layer
- **Platform API:** OS-specific accessibility system

This separation ensures consistency across platforms.

---

## 6.3.3 Role Mapping

Semantic roles map to platform roles.

Examples (conceptual):

- `Button` → UIButton / AXButton / ARIA button
- `Text` → StaticText / AXStaticText / ARIA text
- `Slider` → UISlider / AXSlider / ARIA slider
- `List` → UITable / AXList / ARIA list

Mapping tables are explicit and versioned.

If a platform lacks a direct equivalent:
- the closest semantically correct role is chosen,
- limitations are documented,
- Core semantics remain unchanged.

---

## 6.3.4 Label and Value Mapping

Semantic labels and values map to:

- accessible name,
- accessible description,
- accessible value.

Rules:
- labels map to the primary accessible name,
- values map to platform value fields,
- formatting is applied by the adapter, not Core.

Localization is resolved before reaching the adapter.

---

## 6.3.5 Action Mapping

Semantic actions map to platform affordances.

Examples:
- `Activate` → click / press / default action
- `Increment` → accessibility increment
- `Decrement` → accessibility decrement

Adapters:
- expose only declared actions,
- respect enabled/disabled state,
- route invocations back as Core actions.

No platform-specific actions are invented.

---

## 6.3.6 Focus and Navigation Mapping

Focus semantics map to platform focus systems.

Adapters translate:
- focusable flags,
- traversal order,
- grouping boundaries.

Traversal order from Core IR is preserved unless the platform enforces stricter constraints.
Any deviation is documented and deterministic.

---

## 6.3.7 Geometry and Hit Regions

Accessibility APIs often require geometry.

Adapters use:
- rounded layout geometry,
- hit regions from Core IR,
- paint bounds when required.

No geometry is recomputed or inferred by the adapter.

---

## 6.3.8 Handling Platform Limitations

Platforms differ in capability.

When a platform:
- lacks a feature,
- imposes constraints,
- behaves inconsistently,

the adapter:
- degrades gracefully,
- preserves as much meaning as possible,
- never alters Core semantics.

Unsupported semantics are surfaced in diagnostics.

---

## 6.3.9 Determinism and Testing

Because adapters consume deterministic Core semantics:

- accessibility behavior is reproducible,
- headless tests can validate semantics without adapters,
- platform-specific tests validate adapter correctness.

Adapters are tested independently against semantic fixtures.

---

## 6.3.10 Versioning and Compatibility

Adapter mappings are versioned alongside Core IR.

This ensures:
- semantic meaning remains stable,
- adapter behavior changes are explicit,
- regressions are detectable.

Adapter changes do not redefine semantics.

---

## 6.3.11 Debugging and Inspection

Adapters may attach platform-specific debug metadata:

- mapping decisions,
- degraded behaviors,
- unsupported features.

This metadata is non-semantic and optional.

---

## 6.3.12 Summary

Mapping semantics to platform accessibility APIs:

- preserves Core-defined meaning,
- isolates platform complexity,
- ensures consistency across platforms,
- supports deterministic testing and debugging.

Accessibility correctness is achieved through faithful translation, not inference.

---
