# fission-skia-sys

`fission-skia-sys` is Fission's sole native link authority for Skia. It exposes
a narrow, versioned C ABI instead of C++ or Skia-owned layouts. The initial ABI
contains the production raster foundation: engine and context ownership,
raster surfaces, batched basic frame execution, RGBA readback, memory-pressure
notification, structured diagnostics, and explicit owner-thread checks.

This crate does not use a third-party Rust Skia wrapper. Higher-level rendering,
resource policy, and conversion from Fission's interactive frame belong in
`fission-render-skia`.

## Build modes

Exactly one mode must be selected:

- `skia-prebuilt` (default) consumes a verified Fission artifact from
  `FISSION_SKIA_ARTIFACT_DIR`.
- `skia-build-from-source` consumes the exact pinned checkout from
  `FISSION_SKIA_SOURCE_DIR` and its configured GN output from
  `FISSION_SKIA_BUILD_DIR`, invokes Ninja for the raster library, and compiles
  this crate's bridge against it. It never downloads or configures source
  implicitly; Fission's artifact tooling owns the reproducible GN arguments.
- `test-shim` compiles a small ABI double for ownership/error tests. It is not a
  renderer and must not be shipped as Skia.

The source revision is pinned in `skia_revision.txt`. A source checkout or
prebuilt manifest with another revision is rejected rather than treated as
compatible.
