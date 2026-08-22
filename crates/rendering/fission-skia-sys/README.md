# fission-skia-sys

`fission-skia-sys` is Fission's sole native link authority for Skia. It exposes
a narrow, versioned C ABI instead of C++ or Skia-owned layouts. ABI v14 contains
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
Validated frame fragments can also be recorded into immutable SkPictures and
replayed through the current canvas state. Recording owns every Skia resource
needed after the call and rejects destination-dependent clear and backdrop
operations rather than changing their meaning during later playback.

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

One production mode or the test-only shim must be selected:

- `skia-prebuilt` (default) selects the exact target/profile entry from the
  immutable `artifacts.lock.json` shipped by `fission-skia-artifacts`. It downloads that
  archive over HTTPS once, verifies the locked archive and manifest SHA-256,
  safely extracts it into a content-addressed Cargo cache, verifies every
  payload file plus the Fission version, Skia revision, bridge ABI, target,
  profile, and production-qualification flag, and only then emits link flags.
  A missing or invalid lock entry fails rather than selecting a similar build.
- `skia-build-from-source` consumes the exact pinned checkout from
  `FISSION_SKIA_SOURCE_DIR` and its configured GN output from
  `FISSION_SKIA_BUILD_DIR`, invokes Ninja for the selected profile's complete
  library set (including SVG, paragraph, shaping, and Unicode), and compiles
  this crate's bridge against it. It never downloads or configures source
  implicitly; Fission's artifact tooling owns the reproducible GN arguments.
  Cargo features are additive, so this feature intentionally takes precedence
  when the default `skia-prebuilt` marker is also enabled through a facade or
  platform shell. No downloaded prebuilt is resolved in that case.
- `test-shim` compiles a small ABI double for ownership/error tests. It is not a
  renderer, must not be shipped as Skia, and cannot be combined with either
  production mode.

The source revision is pinned in `skia_revision.txt`. A source checkout or
prebuilt manifest with another revision is rejected rather than treated as
compatible.

`FISSION_SKIA_ARTIFACT_DIR` remains the explicit local/vendor override and is
verified before linking. `FISSION_SKIA_ALLOW_UNQUALIFIED_ARTIFACT=1` applies
only to that explicit override; it cannot weaken a downloaded release entry.
`FISSION_SKIA_CACHE_DIR` changes the persistent cache root. Otherwise the
resolver uses `CARGO_HOME/fission/skia` (or the platform user's `.cargo`
directory). Setting `FISSION_SKIA_OFFLINE=1` or `CARGO_NET_OFFLINE=true`
prohibits network access; offline builds accept only an exact, fully reverified
cache hit or an explicit local/vendor override. Prebuilt resolution never falls
back to a source build.

`FISSION_SKIA_PROFILE` defaults to `native-raster`. The `native-ganesh` profile
selects Vulkan on Linux GNU x86_64/arm64 and Android arm64/armv7/x86_64/x86,
Metal on macOS x86_64/arm64 and the declared iOS device/simulator slices, and
Direct3D 12 on Windows MSVC x86_64/arm64, always with raster fallback. Android
uses API 24 as its minimum Vulkan baseline and links the NDK `libc++_shared`
runtime, which the final APK or AAB must package for each shipped ABI.
Developers select the one profile; the target chooses the vendor backend.
Source mode requires the matching
`fission-skia-build-plan.json` emitted by `tools/skia/skia.py`; it will not label
a raster-configured output as Ganesh. Prebuilt mode likewise requires the exact
profile, target-specific bridge recipe, and native link contract from the
artifact manifest.
Other targets fail clearly until their platform-specific Ganesh surface and
presentation work is implemented.
