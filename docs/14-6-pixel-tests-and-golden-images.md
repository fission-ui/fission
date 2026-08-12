# 14.6 Pixel Tests and Golden Images

This section defines **pixel-based tests** and **golden image testing**.
Pixel tests are supported as a *secondary verification layer*—never as the primary correctness mechanism.
They validate renderer output against pinned references under strictly controlled conditions.

Pixels are evidence, not the source of truth.

---

## 14.6.1 Role of Pixel Tests

Pixel tests exist to:

- validate renderer correctness,
- catch visual regressions missed by structural tests,
- preserve the visual quality of each backend profile,
- verify complex paint effects.

They do **not** replace structural, geometry, or action tests.

---

## 14.6.2 Preconditions for Deterministic Pixel Tests

Pixel tests are only valid when all inputs are pinned.

Required pinning:
- fonts and font versions,
- image decoders,
- color spaces and transfer functions,
- rasterization precision and rounding rules,
- renderer backend and version.

If any input is unpinned, pixel tests are invalid.

---

## 14.6.3 Headless Rasterization Path

Pixel tests use the headless rasterization pipeline.

Rules:
- no OS window or compositor involvement,
- fixed surface size and DPI,
- deterministic raster backend configuration.

Headless rasterization is mandatory for CI.

---

## 14.6.4 Golden Image Format

Golden images are stored in a canonical format.

Properties:
- lossless encoding (e.g. PNG),
- explicit color space metadata,
- versioned metadata header,
- deterministic byte layout.

Golden files are immutable artifacts.

---

## 14.6.5 Golden Generation Workflow

Golden images are generated intentionally.

Workflow:
1. Run tests in “record” mode.
2. Review diffs visually and structurally.
3. Approve and commit goldens explicitly.

Goldens are never updated implicitly.

---

## 14.6.6 Pixel Diffing Strategy

Pixel diffs are structured and explainable.

Diffing may include:
- exact byte comparison,
- per-channel difference maps,
- threshold-based tolerances (explicit and versioned).

Diff output includes visual overlays and statistics.

---

## 14.6.7 Tolerance and Threshold Rules

Tolerance is explicit and rare.

Rules:
- zero tolerance by default,
- non-zero tolerances must be justified,
- tolerances are recorded in test metadata.

Silent fuzziness is forbidden.

---

## 14.6.8 Scoped Pixel Assertions

Pixel tests should be scoped narrowly.

Best practices:
- test isolated components,
- avoid full-screen snapshots when possible,
- combine with structural assertions.

Smaller scopes reduce brittleness.

---

## 14.6.9 Cross-Backend Conformance

The multi-backend qualification suites must compare semantic behavior,
constraints, non-text geometry, and operation coverage across backends. Pixel
goldens must be recorded separately for each backend profile because
rasterization and paragraph metrics may validly differ. The initial Phase 1
work provides structural conformance gates; the full cross-backend and visual
matrix remains migration work.

Examples:
- Vello vs Skia,
- Skia raster vs Skia GPU paths,
- production backends vs the reference conformance renderer,
- platform-specific renderers.

An unexplained change against a backend's own approved golden is a regression.
A difference between two conforming backends is reviewed for quality and
semantic correctness; it is not automatically a bug.

---

## 14.6.10 Animation and Pixel Tests

Animations require special handling.

Rules:
- animations must be frozen at known times,
- time advancement is explicit,
- intermediate frames are tested intentionally.

LiveTest provides deterministic animation capture:

```rust,ignore
use fission::test_driver::{GoldenOptions, LiveTestClient};

let client = LiveTestClient::connect(8787);
client.pause_animations()?;
client.capture_at(160, "actual.png")?;
client.compare_golden(
    "tests/goldens/attention-160ms.png",
    Some("target/attention-160ms.diff.png"),
    GoldenOptions {
        channel_tolerance: 2,
        max_changed_percent: 0.1,
    },
)?;
```

`PauseAnimations`, `ResumeAnimations`, `AdvanceClock`, `CaptureAt`, and
`WaitForIdle` are also available as JSON commands on the LiveTest `/cmd`
endpoint. Repeating motion may be ignored when waiting for idle without being
removed from production code.

---

## 14.6.11 Failure Diagnostics

On failure, pixel tests provide:

- golden image,
- actual render,
- diff image,
- numeric difference summary,
- linked structural snapshot.

Failures are debuggable, not opaque.

`compare_png_to_golden` reports dimensions, changed and total pixel counts,
changed percentage, and maximum channel delta. When a diff path is supplied it
writes a PNG heatmap with changed pixels highlighted in red.

---

## 14.6.12 Performance Considerations

Pixel tests are expensive.

Guidelines:
- keep pixel test count low,
- prefer structural tests,
- shard pixel tests in CI.

Pixel tests are the slowest tier.

---

## 14.6.13 When *Not* to Use Pixel Tests

Avoid pixel tests when:

- asserting layout geometry,
- testing interaction behavior,
- validating accessibility,
- checking animation timing.

Use the appropriate abstraction level.

---

## 14.6.14 Summary

Pixel tests and golden images are effective because:

- they are strictly controlled,
- they validate renderer output,
- they complement higher-level tests,
- they produce explainable failures.

Pixels confirm correctness—they do not define it.

---
