# 9.6 Headless Rasterization

This section defines **headless rasterization**: converting a Display List into pixels without a window system or interactive runtime.
Headless rasterization is essential for CI, golden testing, determinism verification, and offline tooling.

Rasterization answers one question only: *what pixels result from this display list under a fixed configuration?*

---

## 9.6.1 Goals of Headless Rasterization

Headless rasterization exists to:

- run in CI without a windowing system,
- produce deterministic pixel output,
- support golden image testing,
- validate renderers independently,
- enable offline inspection and tooling.

It must behave identically to on-screen rendering under the same configuration.

---

## 9.6.2 Inputs to Headless Rasterization

Rasterization consumes:

- a Display List,
- a fixed viewport size,
- pinned color space and pixel format,
- pinned font and image resources,
- explicit scale factor (e.g. 1.0, 2.0),
- deterministic raster configuration.

All inputs are explicit and versioned.

---

## 9.6.3 Outputs of Headless Rasterization

The output is a **Raster Snapshot**:

- a pixel buffer (e.g. RGBA),
- width, height, stride,
- color space metadata,
- optional damage or coverage metadata.

The snapshot is immutable and serializable.

---

## 9.6.4 Renderer Independence

Headless rasterization targets a renderer interface, not a specific backend.

Rules:
- display lists are consumed verbatim,
- no renderer-specific heuristics are allowed,
- all behavior is driven by display ops.

This allows multiple renderers (e.g. Skia-backed, pure Rust) to be validated against the same inputs.

---

## 9.6.5 Deterministic Raster Configuration

Determinism is enforced by pinning:

- rasterizer implementation and version,
- blending modes and precision,
- antialiasing rules,
- text rasterization parameters,
- image sampling algorithms.

No platform defaults are consulted.

---

## 9.6.6 Fonts and Text Rasterization

Text rasterization is deterministic because:

- font files are bundled and pinned,
- shaping and glyph selection are fixed,
- hinting and subpixel rules are explicit.

Text rendering differences across platforms are eliminated.

---

## 9.6.7 Image Handling

Images are handled deterministically:

- image decoders are pinned,
- color conversion is explicit,
- sampling and filtering are fixed.

Lazy decoding is forbidden in headless mode.

---

## 9.6.8 Clipping and Transparency

Rasterization respects:

- clip ops exactly as specified,
- paint order strictly,
- blending and opacity rules deterministically.

There is no depth buffering or reordering.

---

## 9.6.9 Damage Tracking (Optional)

Headless rasterization may optionally produce:

- per-op coverage maps,
- damage regions between frames.

These artifacts are used for:
- incremental rendering validation,
- performance analysis,
- advanced testing.

They are optional and non-semantic.

---

## 9.6.10 Golden Image Testing

Golden tests compare raster snapshots.

Rules:
- comparisons are byte-for-byte by default,
- tolerances must be explicit and justified,
- failures include diff visualizations and metadata.

Golden tests validate the full pipeline end-to-end.

---

## 9.6.11 Performance Considerations

Headless rasterization prioritizes correctness over speed.

However:
- pipelines are optimized for batch execution,
- memory reuse is allowed where deterministic,
- parallelism is allowed only if deterministic.

CI throughput is considered but secondary.

---

## 9.6.12 Error Handling

Rasterization errors include:

- unsupported display ops,
- invalid geometry,
- missing resources,
- configuration mismatches.

Errors are deterministic and include provenance and op indices.

---

## 9.6.13 Relationship to On-Screen Rendering

On-screen rendering must:

- share the same display list consumption logic,
- differ only in presentation (swapchain, vsync),
- match headless output exactly for identical inputs.

Any divergence is a renderer bug.

---

## 9.6.14 Summary

Headless rasterization:

- turns display lists into deterministic pixels,
- enables CI and golden testing,
- validates renderers independently,
- completes the testing story for visuals.

It is the final step that makes visuals testable with confidence.

---
