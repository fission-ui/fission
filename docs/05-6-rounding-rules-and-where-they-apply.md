# 5.6 Rounding Rules and Where They Apply

This section defines how numeric rounding is handled throughout the pipeline and, critically, *where* rounding is permitted to occur.
Rounding is a major source of nondeterminism in UI systems if not tightly controlled.

In this framework, rounding is explicit, centralized, and observable.

---

## 5.6.1 Why Rounding Must Be Explicit

Rounding affects:

- layout geometry,
- paint bounds,
- hit-testing,
- pixel alignment,
- snapshot comparisons.

If rounding:
- occurs implicitly,
- varies by platform,
- depends on floating-point quirks,

then deterministic behavior is impossible.

Therefore, rounding is treated as a semantic concern, not a rendering detail.

---

## 5.6.2 Numeric Domains in the Pipeline

The pipeline distinguishes between numeric domains:

1. **Logical Units**  
   - Used during authoring, lowering, and layout.
   - Represent abstract, device-independent coordinates.
   - May be fractional.

2. **Device Units**  
   - Used during rendering and rasterization.
   - Represent concrete pixel-aligned coordinates.

Rounding defines the boundary between these domains.

---

## 5.6.3 Where Rounding Is Forbidden

Rounding must **not** occur during:

- authoring widget construction,
- lowering or desugaring,
- canonicalization,
- layout constraint resolution,
- baseline computation.

All of these stages operate in logical units and preserve exact values.

Implicit rounding at these stages is a correctness bug.

---

## 5.6.4 Where Rounding Is Permitted

Rounding is permitted only at explicit, documented boundaries:

1. **Layout Finalization**  
   - When producing a layout snapshot intended for rendering.
   - Converts logical geometry into render-ready geometry.

2. **Rendering Backend Translation**  
   - When mapping geometry to backend-specific APIs.
   - Must follow the same rounding policy as layout finalization.

These boundaries are explicit and testable.

---

## 5.6.5 Rounding Policy Configuration

Rounding behavior is governed by an explicit policy.

A rounding policy defines:
- rounding mode (e.g. floor, ceil, nearest),
- axis-specific behavior,
- bias rules for half values.

The policy:
- is part of runtime configuration,
- is included in test harnesses,
- is fixed for deterministic runs.

Changing the policy is a semantic change.

---

## 5.6.6 Canonical Representation of Rounded Values

Rounded values are part of observable state.

Rules:
- rounded geometry is stored explicitly in layout snapshots,
- tests assert against rounded values, not raw floats,
- snapshots never rely on implicit renderer rounding.

This ensures headless and rendered results match.

---

## 5.6.7 Rounding and Hit Testing

Hit testing uses rounded geometry.

Reasons:
- input events originate in device space,
- hit regions must align with rendered pixels,
- mismatch causes flaky interaction tests.

The same rounded geometry is used for:
- painting,
- hit testing,
- paint bounds computation.

---

## 5.6.8 Floating-Point Determinism

Floating-point operations are constrained:

- well-defined operation order,
- no reliance on platform-specific math intrinsics,
- no accumulation of error across frames.

Where necessary:
- fixed-point representations may be used,
- or explicit rounding steps inserted.

Determinism takes precedence over micro-precision.

---

## 5.6.9 Testing Rounding Behavior

Rounding behavior is tested explicitly:

- golden layout snapshots include rounded geometry,
- tests assert exact rects and bounds,
- headless and rendered output must agree.

Rounding-related regressions are detectable immediately.

---

## 5.6.10 Error Handling

Invalid rounding configurations include:

- unspecified rounding policy,
- inconsistent policy across pipeline stages,
- backend-specific overrides.

Such configurations are rejected at startup or pipeline initialization.

---

## 5.6.11 Summary

Rounding rules are:

- explicit and centralized,
- forbidden in semantic phases,
- permitted only at defined boundaries,
- governed by a stable policy,
- fully observable and testable.

By controlling rounding rigorously, the framework eliminates a major source of nondeterminism.

---
