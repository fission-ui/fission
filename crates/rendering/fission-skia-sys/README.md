# fission-skia-sys

`fission-skia-sys` is Fission's sole native link authority for Skia. It exposes
a narrow, versioned C ABI instead of C++ or Skia-owned layouts. ABI v7 contains
the production raster foundation: engine and context ownership, raster
surfaces, batched paint state and shape execution, RGBA readback,
memory-pressure notification, structured diagnostics, and explicit
owner-thread checks. It also carries retained SkParagraph paint data and
bounded opacity layers whose contents are isolated before group alpha is
applied once. Immutable images are decoded through a mandatory caller byte
limit, retain their oriented N32 sRGB pixels, and draw with explicit source,
destination, and nearest-or-linear sampling semantics.
Backdrop Gaussian blur is an atomic operation over explicit physical bounds,
with rounded-corner clipping and deterministic identity behavior at zero sigma.
Retained SVG documents are parsed as bounded UTF-8 into SkSVGDOM with external
resources disabled, then rendered atomically into an explicit destination.
Intrinsic documents use centered contain placement; percentage-sized roots use
the destination as their SkSVGDOM container viewport so viewBox and
preserveAspectRatio remain document-owned.

The paint contract carries finite unpremultiplied sRGB values. Gradients use
shape-resolved coordinates and ordered stops. Empty gradients are transparent,
one-stop gradients are solid, coincident stops retain their hard-stop order via
adjacent representable offsets, and zero-radius or coincident-endpoint
gradients resolve to their terminal stop. Odd dash arrays are repeated to an
even count; empty and all-zero arrays are solid strokes. These edge cases are
explicit so Skia never turns an accepted paint into an accidental no-op.

This crate does not use a third-party Rust Skia wrapper. Higher-level rendering,
resource policy, and conversion from Fission's interactive frame belong in
`fission-render-skia`.

## Build modes

Exactly one mode must be selected:

- `skia-prebuilt` (default) consumes a verified Fission artifact from
  `FISSION_SKIA_ARTIFACT_DIR`.
- `skia-build-from-source` consumes the exact pinned checkout from
  `FISSION_SKIA_SOURCE_DIR` and its configured GN output from
  `FISSION_SKIA_BUILD_DIR`, invokes Ninja for the complete native-raster
  library set (including SVG, paragraph, shaping, and Unicode), and compiles
  this crate's bridge against it. It never downloads or configures source
  implicitly; Fission's artifact tooling owns the reproducible GN arguments.
- `test-shim` compiles a small ABI double for ownership/error tests. It is not a
  renderer and must not be shipped as Skia.

The source revision is pinned in `skia_revision.txt`. A source checkout or
prebuilt manifest with another revision is rejected rather than treated as
compatible.
