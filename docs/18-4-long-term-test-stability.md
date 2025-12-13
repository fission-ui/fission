# 18.4 Long-Term Test Stability

This section defines how the framework ensures **tests remain valid, non-flaky, and meaningful**
over long time horizons—even as the system evolves.
Test stability is treated as an architectural requirement, not a tooling concern.

If tests rot, the architecture has failed.

---

## 18.4.1 Why Long-Term Test Stability Matters

The framework is built around tests that:

- verify geometry and layout precisely,
- assert semantic meaning and accessibility,
- validate action dispatch and state transitions,
- detect regressions through snapshots and diffs.

These tests must remain trustworthy across:
- refactors,
- performance optimizations,
- renderer changes,
- platform additions.

---

## 18.4.2 Determinism as the Foundation

Test stability is anchored in determinism.

Guarantees:
- identical inputs produce identical Core snapshots,
- time is owned and controlled,
- fonts, rounding, and traversal are pinned,
- no platform timing or ordering leaks into results.

Flakiness is structurally eliminated.

---

## 18.4.3 Stable Test APIs

Test APIs are part of the compatibility surface.

Rules:
- selectors (`find(...)`) are stable and versioned,
- geometry queries (`rect`, `baseline`, `paint_bounds`) have fixed semantics,
- error messages are structured and diffable.

Tests do not depend on incidental structure.

---

## 18.4.4 Snapshot Versioning and Migration

Snapshots evolve safely.

Rules:
- snapshots declare explicit format versions,
- older snapshots remain readable,
- migrations are deterministic and tool-assisted,
- snapshot diffs remain meaningful across versions.

Snapshots never silently change meaning.

---

## 18.4.5 Renderer-Independence of Tests

Most tests are renderer-agnostic.

Strategies:
- test geometry and semantics before rasterization,
- validate display lists instead of pixels where possible,
- reserve pixel tests for renderer validation only.

This isolates renderer churn from application tests.

---

## 18.4.6 Scoped and Intentional Assertions

Tests should assert intent, not implementation.

Guidelines:
- assert relative geometry where possible,
- avoid asserting exact child indices unless necessary,
- prefer semantic selectors over structural paths.

Tests describe *what must hold*, not *how it is implemented*.

---

## 18.4.7 Golden Files and Snapshot Discipline

Golden artifacts are managed carefully.

Rules:
- goldens are version-pinned,
- updates require explicit review,
- diffs are human-readable,
- mass updates are discouraged.

Goldens document expectations, not current behavior.

---

## 18.4.8 Handling Intentional Changes

When behavior changes intentionally:

- tests fail loudly,
- failures include structured diffs,
- migration tools assist updates,
- change logs document impact.

Silently updating tests is forbidden.

---

## 18.4.9 Time and Animation Stability

Animated tests are stable because:

- time advancement is explicit,
- frame stepping is deterministic,
- easing functions are pure and pinned.

Animations are testable like pure functions.

---

## 18.4.10 Accessibility Test Stability

Accessibility tests are stable because:

- semantics are explicit data,
- traversal order is canonical,
- platform mappings are tested separately.

Accessibility regressions are detectable early.

---

## 18.4.11 CI and Environment Pinning

CI environments are pinned.

Practices include:
- pinned OS and browser versions,
- pinned font and locale bundles,
- fixed DPI and surface sizes.

Environmental drift is controlled.

---

## 18.4.12 Avoiding Test Fragility

Anti-patterns are discouraged:

- pixel-perfect assertions where geometry suffices,
- reliance on implicit ordering,
- timing-based waits.

Framework APIs make correct testing easier than fragile testing.

---

## 18.4.13 Tooling Support for Stability

Tooling reinforces stability.

Tools provide:
- snapshot diff visualizers,
- geometry mismatch explanations,
- action trace replays,
- migration assistants.

Stable tests are easier to maintain.

---

## 18.4.14 Long-Term Guarantees

The framework commits to:

- stable test APIs within major versions,
- readable failures across versions,
- migration support for breaking changes.

Tests are assets, not liabilities.

---

## 18.4.15 Summary

Long-term test stability is achieved because:

- determinism is architectural,
- tests observe explicit data,
- evolution is versioned and opt-in,
- tooling supports change responsibly.

As the framework evolves, tests continue to mean the same thing.
