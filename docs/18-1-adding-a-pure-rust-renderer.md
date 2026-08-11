# 18.1 Adding a Pure Rust Renderer

This section describes how a **pure Rust renderer** must be added as a
first-class backend without modifying application widgets or Core semantics. A
production implementation must use Fission-owned paragraph, frame, resource,
capability, diagnostic, and graphics-session contracts rather than exposing its
own types upward.

Rendering is being made replaceable behind Fission-owned contracts; semantics
are not backend-owned.

---

## 18.1.1 Motivation

A pure Rust renderer is desirable to:

- reduce external dependencies,
- enable full Rust end-to-end builds,
- improve portability and auditability,
- support environments where Skia is unsuitable,
- deepen control over determinism and precision.

No backend is a permanent reference oracle. The current Vello-centered stack,
the planned production Skia implementation, software profiles, and future
Rust-native renderers must be qualified against shared semantic contracts and
their own visual baselines.

---

## 18.1.2 Architectural Constraints

Any renderer must obey strict constraints:

- consume immutable Fission interactive frames and resource snapshots,
- obtain text layout, hit testing, selection, and caret geometry from the
  paragraph result selected for the same backend profile,
- never own time or scheduling,
- never infer semantics,
- never mutate Core state.

Violation of these rules is a correctness bug.

---

## 18.1.3 Renderer Interface Contract

The multi-backend refactor is introducing a renderer-facing contract with:

- immutable scene and frame input,
- explicit surface lifecycle, size, scale, and pixel format,
- logical resource snapshots and typed external-surface bindings,
- declared capabilities and mandatory conformance validation,
- deterministic command ordering and diagnostic provenance.

The renderer returns:
- render and presentation reports,
- optional readback pixels,
- explicit lifecycle, capability, and recovery errors,
- cache and memory diagnostics.

There is no callback into the Core.

---

## 18.1.4 CPU-First Rendering Model

The pure Rust renderer is expected to be CPU-first.

Characteristics:
- software rasterization,
- predictable floating-point behavior,
- deterministic scan conversion,
- explicit color space handling.

CPU rendering is ideal for tests and headless execution.

---

## 18.1.5 Incremental Rendering Opportunities

Incrementality is optional but encouraged.

Strategies include:
- reusing unchanged display list spans,
- tile-based invalidation,
- cached text and path rasterization.

Incrementality must not alter output.

---

## 18.1.6 Text Rendering and Fonts

Text rendering is the hardest component.

Approach:
- pinned font versions,
- deterministic shaping and rasterization,
- explicit glyph cache management,
- no platform font fallback.

Text output must be deterministic for a pinned paragraph/backend profile.
Different conforming text engines may produce different valid metrics and
rasterization, so their golden images are versioned per profile.

---

## 18.1.7 Path, Shape, and Image Rendering

The renderer must support:

- rectangles and rounded rectangles,
- vector paths,
- strokes and fills,
- images with explicit sampling rules.

All geometry must follow Core rounding rules.

---

## 18.1.8 Color and Blending

Color handling rules:

- explicit color spaces,
- deterministic blending equations,
- no reliance on GPU-specific behavior,
- reproducible alpha compositing within the pinned backend profile.

Blending must satisfy Fission's semantic operation contract. Cross-backend
pixel identity is not required.

---

## 18.1.9 Performance Considerations

A pure Rust renderer prioritizes correctness first.

Performance strategies include:
- SIMD where deterministic,
- multi-threaded tiling (optional),
- cache-friendly buffers.

Performance tuning must preserve semantic output and the backend profile's
documented visual tolerances.

---

## 18.1.10 Testing the Renderer

Production renderer correctness must be validated via:

- golden image tests,
- shared semantic and non-text geometric conformance tests,
- backend-specific visual golden tests,
- headless CI execution,
- stress tests with complex scenes.

The same frame must be accepted or rejected according to truthful declared
capabilities. Conforming backends may differ visually while remaining stable
against their own approved baselines.

---

## 18.1.11 Deployment Strategy

The Rust renderer can be deployed as:

- default headless renderer,
- optional production backend,
- fallback when GPU is unavailable.

Selection is explicit and configurable.

---

## 18.1.12 Coexistence With Skia

Multiple renderers must be able to coexist safely.

Rules:
- frame and resource contracts are backend-agnostic,
- shared semantic invariants are validated across backends,
- visual output is qualified per backend,
- an unsupported operation is rejected with provenance rather than omitted.

---

## 18.1.13 Why This Is Feasible

This is feasible because:

- the Core owns all semantics,
- display lists are deterministic data,
- rendering is isolated,
- the architecture forbids leakage.

Rendering complexity is contained.

---

## 18.1.14 Summary

Adding a pure Rust renderer is safe because:

- the Core Runtime is unchanged,
- renderer contracts are strict,
- profile-scoped determinism is enforceable,
- shared contract tests can enforce semantic behavior, while the complete
  cross-backend and visual qualification matrix remains required work for each
  production profile.

This enables a fully Rust-native UI stack without architectural compromise.
