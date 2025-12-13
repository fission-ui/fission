# 18.1 Adding a Pure Rust Renderer

This section describes how a **pure Rust renderer** can be added as a first-class backend
without modifying the Core Runtime, Core IR, or authoring layers.
The renderer is an interchangeable implementation detail that consumes display lists and produces pixels.

Rendering is replaceable; semantics are not.

---

## 18.1.1 Motivation

A pure Rust renderer is desirable to:

- reduce external dependencies,
- enable full Rust end-to-end builds,
- improve portability and auditability,
- support environments where Skia is unsuitable,
- deepen control over determinism and precision.

The initial Skia renderer remains the reference implementation.

---

## 18.1.2 Architectural Constraints

Any renderer must obey strict constraints:

- consume **display lists only**,
- never perform layout or hit testing,
- never own time or scheduling,
- never infer semantics,
- never mutate Core state.

Violation of these rules is a correctness bug.

---

## 18.1.3 Renderer Interface Contract

The Core exposes a renderer-facing contract:

- immutable display list input,
- explicit surface size and pixel format,
- deterministic command ordering,
- optional debug metadata (behind flags).

The renderer returns:
- rendered pixels or GPU submission,
- explicit error signals.

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

Text determinism is mandatory for pixel tests.

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
- reproducible alpha compositing.

Blending behavior must match reference renderers.

---

## 18.1.9 Performance Considerations

A pure Rust renderer prioritizes correctness first.

Performance strategies include:
- SIMD where deterministic,
- multi-threaded tiling (optional),
- cache-friendly buffers.

Performance tuning must preserve bitwise output stability.

---

## 18.1.10 Testing the Renderer

Renderer correctness is validated via:

- golden image tests,
- cross-renderer parity tests,
- headless CI execution,
- stress tests with complex scenes.

The same display list must render identically across backends.

---

## 18.1.11 Deployment Strategy

The Rust renderer can be deployed as:

- default headless renderer,
- optional production backend,
- fallback when GPU is unavailable.

Selection is explicit and configurable.

---

## 18.1.12 Coexistence With Skia

Multiple renderers coexist safely.

Rules:
- display list format is renderer-agnostic,
- behavior is validated against Skia,
- discrepancies are treated as bugs.

Skia remains a reference oracle.

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
- determinism is enforceable,
- testing infrastructure already exists.

This enables a fully Rust-native UI stack without architectural compromise.
