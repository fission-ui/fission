# 1.6 Non-Goals and Explicit Trade-Offs

This section documents what the framework explicitly does **not** aim to do, and the trade-offs that are intentionally made.
Making non-goals explicit is critical to preserving architectural clarity and avoiding gradual erosion of core principles.

If a proposal advances a non-goal at the expense of a stated goal, it should be rejected or re-scoped.

---

## 1.6.1 Non-Goal: Mimicking Web Standards or HTML/CSS Semantics

The framework does not attempt to replicate:
- HTML layout rules,
- CSS box model semantics,
- browser DOM behavior,
- web accessibility quirks.

While the web is a supported platform, its standards are not the conceptual model.

Rationale:
- Web standards carry decades of legacy behavior.
- They are often underspecified or browser-dependent.
- They complicate determinism and testability.

Instead, the framework defines its own clear, deterministic layout and semantics model.

---

## 1.6.2 Non-Goal: Immediate-Mode or Retained Hybrid APIs

The framework does not support:
- immediate-mode UI APIs,
- imperative drawing during render passes,
- mutating UI state during layout or paint.

Rationale:
- Immediate-mode APIs obscure intent.
- They make it difficult to capture structured snapshots.
- They complicate deterministic replay.

All UI is expressed as data and lowered before rendering.

---

## 1.6.3 Non-Goal: Implicit Side Effects

The framework avoids:
- hidden global state,
- implicit access to system time,
- implicit access to random number generators,
- side effects during widget construction or layout.

Rationale:
- Implicit side effects are a primary source of nondeterminism.
- They undermine testability and replay.

All effects must be explicit and observable.

---

## 1.6.4 Non-Goal: Unlimited Extensibility at the Core Level

The Core IR is intentionally closed.

The framework does not aim to:
- allow arbitrary user-defined Core primitives,
- dynamically extend Core semantics at runtime.

Rationale:
- An open Core leads to fragmentation and instability.
- Deterministic testing requires a known semantic set.

Extensibility occurs in the authoring layer through desugaring.

---

## 1.6.5 Non-Goal: Hiding Complexity Behind “Magic”

The framework does not attempt to:
- automatically infer intent without explicit data,
- hide layout or rendering behavior behind heuristics,
- “guess” user expectations.

Rationale:
- Magic reduces predictability.
- Explicitness improves debuggability and testability.

Where complexity exists, it is surfaced in structured form.

---

## 1.6.6 Trade-Off: Slightly Higher Initial Conceptual Overhead

The framework accepts:
- a more explicit mental model,
- a learning curve around Core IR and actions.

In exchange, it gains:
- long-term stability,
- reliable testing,
- confidence during refactoring.

This trade-off favors serious applications and teams over minimal tutorials.

---

## 1.6.7 Trade-Off: Structured Tests Over Visual Intuition

The framework prioritizes:
- structural and semantic assertions,
- deterministic geometry checks,

over:
- purely visual, pixel-based intuition.

Pixel tests are supported, but they are not the primary correctness mechanism.

This improves reliability at the cost of initial familiarity.

---

## 1.6.8 Trade-Off: Platform Parity Over Native Quirks

The framework favors:
- consistent behavior across platforms,

over:
- exploiting every platform-specific quirk or feature.

Where platform-specific behavior is required, it is isolated behind explicit APIs.

---

## 1.6.9 Trade-Off: Stability Over Rapid API Experimentation

The Core IR and determinism guarantees impose constraints on change.

As a result:
- Core changes require careful review,
- behavior changes must be versioned or documented.

This slows experimentation at the lowest level but preserves trust and compatibility.

---

## 1.6.10 Summary

Explicit non-goals and trade-offs protect the framework’s core values.

By clearly stating what the framework does *not* attempt to do, we:
- reduce ambiguity,
- guide decision-making,
- prevent architectural drift.

These constraints are deliberate and foundational.

---
