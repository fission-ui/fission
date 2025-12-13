# 1.2 Determinism as a First-Class Requirement

Determinism is a foundational requirement of the framework. It is not an optimization, a testing convenience, or a development-mode feature. The framework is designed so that determinism is the default behavior in both production and test environments.

This section defines what determinism means in practice, why it matters, and how it is enforced.

---

## 1.2.1 Definition of Determinism

The framework is deterministic if, for a given set of inputs, it produces identical outputs across runs.

**Inputs include:**
- Application code and dependencies
- Initial application state
- User input events and their ordering
- Time progression
- Configuration (fonts, DPI, rounding rules, feature flags)

**Outputs include:**
- Core IR
- Layout tree (geometry, baselines, bounds)
- Semantic tree
- Display list and paint ordering
- Event and action traces
- Rendered output (given the same renderer backend)

Any divergence in these outputs given identical inputs is a determinism violation.

---

## 1.2.2 Why Determinism Matters

Determinism enables capabilities that are otherwise impractical or unreliable:

- **Non-flaky tests:** UI tests can be trusted and run at scale in CI.
- **Exact replay:** Bugs can be reproduced precisely from recorded traces.
- **Meaningful diffs:** Structural snapshots can be compared across versions.
- **Automated reasoning:** Tools and LLMs can analyze UI behavior reliably.
- **Refactoring confidence:** Rendering and layout internals can change without altering observable behavior.

Without determinism, UI testing degrades into heuristic pixel comparisons and manual inspection.

---

## 1.2.3 Owned Time Model

The framework owns time.

- There is no implicit access to system clocks during layout, rendering, or animation.
- All time-dependent behavior is driven by an explicit `Clock` owned by the runtime.

### Clock Properties
- Monotonic
- Explicitly advanced by the runtime or test harness
- Serializable for replay

### Implications
- Animations advance deterministically.
- Media playback can be mocked or controlled in tests.
- Tests can advance time precisely without sleeping.

---

## 1.2.4 Controlled Sources of Nondeterminism

The framework explicitly controls or eliminates common sources of nondeterminism:

### Fonts
- Fonts must be bundled and pinned.
- System font discovery is disallowed by default.
- Font selection and fallback order are deterministic.

### Floating-Point Behavior
- Layout uses well-defined rounding and snapping rules.
- Rounding policy is configurable but explicit.
- Rounding is applied at defined pipeline stages only.

### Traversal Order
- Tree traversal order is explicit and stable.
- Maps and sets must be normalized into ordered collections before lowering.

### Input Events
- Input events are queued and processed in a deterministic order.
- Event dispatch paths are recorded and inspectable.

### Parallelism
- Any parallel computation must produce deterministic results.
- Reduction and merging steps must be order-independent or explicitly ordered.

---

## 1.2.5 Rounding and Snapping Rules

Rounding is a frequent source of subtle nondeterminism.

The framework defines explicit rounding stages:
1. Logical layout space (floating point)
2. Optional snapping to logical pixels
3. Optional snapping to device pixels (based on DPR)

The rounding policy is:
- configured at runtime,
- included in snapshots,
- identical across platforms given the same configuration.

No implicit rounding is allowed inside layout algorithms.

---

## 1.2.6 Determinism Across Platforms

The Core Runtime must behave identically across all supported platforms.

- Platform-specific APIs must not influence layout, semantics, or event routing.
- Rendering backends may differ internally but must consume identical display lists.

Platform shells are responsible only for:
- input collection,
- surface creation,
- accessibility bridging.

All observable behavior originates from the Core.

---

## 1.2.7 Determinism in Tests

Tests rely on determinism to be meaningful.

The test harness:
- runs headlessly,
- uses a fixed viewport and DPR,
- pins fonts and resources,
- controls the clock,
- captures structured snapshots.

Tests should assert:
- structure and geometry,
- semantics and roles,
- actions and state transitions,
- temporal behavior.

Pixel comparison is optional and secondary.

---

## 1.2.8 Handling Non-Deterministic Subsystems

Some subsystems cannot be made fully deterministic (e.g., hardware video decoding).

The framework handles these by:
- isolating them behind explicit boundaries,
- providing deterministic mock implementations,
- exposing observable state in snapshots.

Non-deterministic behavior must never leak into the Core Runtime.

---

## 1.2.9 Determinism as a Compatibility Contract

Determinism is part of the framework’s compatibility contract.

Changes that alter:
- layout results,
- semantic structure,
- paint ordering,
- event traces,

must be treated as breaking changes unless explicitly versioned and documented.

---

## 1.2.10 Summary

Determinism underpins:
- reliable testing,
- cross-platform consistency,
- long-term maintainability,
- automated reasoning.

Every architectural choice in this framework is evaluated against its impact on determinism.

---
