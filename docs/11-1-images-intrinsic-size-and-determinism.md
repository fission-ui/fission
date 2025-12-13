# 11.1 Images (Intrinsic Size and Determinism)

This section defines how image embeds behave with respect to intrinsic size, layout, painting, and determinism.
Images are treated as deterministic resources whose geometry and visual contribution are fully specified by data, not platform behavior.

An image must never change layout or paint output nondeterministically.

---

## 11.1.1 Image as a Deterministic Resource

Images are modeled as immutable resources.

Each image resource has:
- a stable identifier,
- pinned binary content (or hash),
- explicit pixel dimensions,
- explicit color space and format.

Image identity is content-addressed, not path-addressed.

---

## 11.1.2 Intrinsic Size Definition

An image’s intrinsic size is defined as:

- its decoded pixel width and height,
- expressed in logical units via an explicit scale factor.

Rules:
- intrinsic size is known before layout,
- intrinsic size does not depend on device DPI,
- intrinsic size is cached deterministically.

---

## 11.1.3 Explicit Scale and DPI Handling

Images do not implicitly adapt to device DPI.

Rules:
- scale factors are explicit inputs,
- logical size = pixel size ÷ scale,
- changing scale is a deterministic configuration change.

This avoids platform-dependent image sizing.

---

## 11.1.4 Layout Participation

Images participate in layout like other nodes.

They may:
- use intrinsic size,
- be constrained by parent layout,
- preserve aspect ratio explicitly,
- be stretched or clipped by configuration.

No implicit “fit” behavior exists.

---

## 11.1.5 Aspect Ratio Handling

Aspect ratio behavior is explicit.

Supported modes include:
- preserve (default),
- stretch,
- letterbox,
- crop.

Aspect ratio decisions are resolved during layout, not painting.

---

## 11.1.6 Image Decoding Determinism

Image decoding is deterministic because:

- decoders are pinned and versioned,
- color conversion is explicit,
- metadata interpretation is fixed,
- lazy decoding is forbidden in deterministic paths.

Decoded pixel data is identical across platforms.

---

## 11.1.7 Painting Images

Image painting emits explicit draw-image ops.

Each op includes:
- resolved geometry,
- sampling mode,
- color space,
- paint bounds.

Renderers are not allowed to reinterpret image data.

---

## 11.1.8 Sampling and Filtering

Sampling behavior is explicit.

Rules:
- nearest / linear / cubic are explicit choices,
- no platform default filtering is used,
- filtering decisions are deterministic.

Sampling affects paint bounds conservatively.

---

## 11.1.9 Clipping and Visibility

Images respect clip bounds like any other node.

Rules:
- clipping does not alter intrinsic size,
- clipped regions do not emit paint ops,
- visibility is derived from clip bounds.

---

## 11.1.10 Headless Testing of Images

Images are testable headlessly.

Tests may assert:
- intrinsic size,
- layout rects,
- emitted paint ops,
- raster output via golden tests.

Stub images with fixed content are encouraged.

---

## 11.1.11 Error Handling and Fallbacks

Image loading errors are explicit states.

Rules:
- failed images have deterministic fallback size,
- fallback visuals are explicit,
- errors do not crash layout or paint.

---

## 11.1.12 Performance Considerations

Determinism does not preclude performance.

Optimizations include:
- decode caching,
- shared pixel buffers,
- atlas packing (renderer-side).

Optimizations must not affect observable behavior.

---

## 11.1.13 Summary

Image embeds are deterministic because:

- intrinsic size is explicit,
- decoding and sampling are pinned,
- layout and paint behavior are data-driven,
- errors and fallbacks are explicit.

Images behave like geometry—not surprises.

---
