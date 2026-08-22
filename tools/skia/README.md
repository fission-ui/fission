# Fission Skia artifact foundation

This directory owns the reproducible-input and artifact-layout contract for the
direct Fission Skia integration. It does not contain or advertise a production
artifact yet.

The current direct bridge contract is ABI v14. ABI changes are explicit artifact
identity changes; the tooling will not package or verify a header from another
bridge ABI.

The source pin is the immutable upstream commit
`cf5c36972b73698eb3939cda147ea47152670312`. The upstream
`canvaskit/0.41.0` tag and milestone 148 are recorded only as useful review
hints. They are not selection inputs and the pin is explicitly unqualified.

`crates/rendering/fission-skia-artifacts/artifacts.lock.json` is intentionally empty.
It is the single artifact-selection authority bundled into the published crate.
Adding an entry is a release operation that must happen only after the
corresponding archive, provenance, and qualification evidence exist.

## Supported vocabulary

`config.json` declares these profiles:

- `native-raster`, whose foundation build recipe is available;
- `native-ganesh`, target-selected as Vulkan on Linux GNU x86_64/arm64, Metal
  on macOS x86_64/arm64 plus iOS device/simulator slices, and Direct3D 12 on
  Windows MSVC x86_64/arm64, plus Vulkan on all four declared Android ABIs;
- `native-graphite-qualification`, planned and never an implicit fallback;
- `canvaskit-production`, with WebGL/Ganesh plus Skia raster fallback;
- `canvaskit-software-qualification`, a CPU-only Skia raster comparison lane.

Linux GNU and musl, macOS, Windows MSVC, Android ABIs, iOS device/simulator
slices, and interactive Web target names are declared. Declared means the tools
recognize the identity; it does not mean an artifact exists or is qualified.
Profiles without an implemented recipe fail rather than producing a plausible
but incomplete archive. `native-ganesh` classifies every declared native
target. Linux musl is explicitly unsupported until its C++/fontconfig
toolchain is reproducible.

The Linux recipe enables Vulkan and VMA while disabling GL, X11, Metal,
Direct3D, Dawn, and Graphite. Its exact native consumer link contract is `dl`,
`fontconfig`, and the Vulkan loader. The macOS and iOS recipes enable Metal,
compile their AppKit/UIKit presenter units, and bind the exact Apple framework
set in the artifact receipt. Raster `SkSurface` remains in every native Ganesh
artifact as the fallback. The Windows recipes enable Direct3D 12, compile the
DXGI presenter units, and link exactly `d3d12`, `dxgi`, `user32`, and
`kernel32`. Each target recipe owns its exact bridge sources,
backend define, GN arguments, and native link contract; developers still
select only `native-ganesh`, never a vendor profile.

The Android recipes use Vulkan 1.0 with API 24 as the minimum deployment
baseline. Their proven consumer link contract is `android`, `vulkan`, and the
NDK `c++_shared` runtime; `log`, `dl`, EGL, GLES, and JNI are not inferred.
Deployment metadata must record `cxx_runtime` as `libc++_shared`, and the final
APK or AAB must package the matching NDK `libc++_shared.so` ABI slice.

## Local source and vendor overrides

The tools never fetch Skia and never silently build it from source. Prepare an
exact checkout separately, including its pinned dependencies, then use:

```sh
python3 tools/skia/skia.py build-native \
  --source-dir /absolute/path/to/skia \
  --build-dir /absolute/path/to/skia-out \
  --profile native-raster \
  --target x86_64-unknown-linux-gnu \
  --toolchain-id clang-20-linux-x86_64 \
  --gn-sha256 "$PINNED_GN_SHA256" \
  --ninja-sha256 "$PINNED_NINJA_SHA256"
```

Select the target-native GPU build by changing `--profile` to `native-ganesh`.
Both source and prebuilt consumers select it with
`FISSION_SKIA_PROFILE=native-ganesh`; omitting the variable continues to select
`native-raster`.

`FISSION_SKIA_SOURCE_DIR` and `FISSION_SKIA_BUILD_DIR` are the equivalent
explicit environment overrides. If a command-line value and environment value
conflict, the command fails. A Git checkout must be clean and exactly at the
pin. A source vendor directory without `.git` must contain
`FISSION_SKIA_SOURCE_REVISION` whose only line is the exact pinned commit.

Android and iOS recipes require their target-specific GN inputs explicitly,
for example `--gn-arg ndk=/absolute/path --gn-arg ndk_api=24` or
`--gn-arg ios_min_target=13.0`. Extra arguments
are accepted only when the selected target declares them in
`allowed_gn_overrides`, and are recorded in the build plan. Desktop targets
currently accept no overrides. Overrides cannot replace profile-owned
arguments; changing one of those requires reviewing `config.json`.

The build command invokes only the already-present GN and Ninja executables. It
does not run dependency synchronization. `--gn-sha256` and `--ninja-sha256`
must be independently pinned lowercase SHA-256 values; do not derive them from
the executable during the build invocation. Both tools are hashed and matched
before either is executed, including for `--version`, and each is checked again
immediately before its build command. It builds the profile's complete, ordered
upstream static-library set and writes `fission-skia-build.json` as a strict
build receipt. The receipt binds the canonical build plan, each tool's expected
and observed digest, normalized versions, source identity, and every upstream
output's size and SHA-256. Tool and source identities intentionally omit host
paths. The receipt also binds the selected bridge source list and preprocessor
defines. The Fission bridge is built from the bridge sources owned by
`fission-skia-sys` and supplied to the packaging step; packaging checks its
header ABI and binds the header, library digest, sources, and defines into the
artifact manifest. A raster bridge receipt therefore cannot be relabelled as a
Ganesh artifact.

Native notice bundles follow the exact selected build recipe. Every native
profile includes Fission, Skia, Expat, FreeType, HarfBuzz, ICU,
libjpeg-turbo, libpng, libwebp, Wuffs, and zlib. Android targets additionally
include the NDK cpu-features notice. Vulkan Ganesh recipes on Linux and Android
also include the Vulkan Headers and Vulkan Memory Allocator notices; Metal and
Direct3D recipes do not claim code they did not build. The selected notice set,
native link libraries, and frameworks are bound into the strict build receipt,
so packaging cannot relabel a receipt with a different target contract.

CanvasKit has a separate offline command because its Emscripten toolchain is
part of the artifact identity. It accepts only the two declared profiles and
requires a clean or receipted Skia checkout, a clean or receipted emsdk checkout
at `c69d433d8509c5c64564c2f0d054bf102a5cf67e`, Emscripten 4.0.7, and
independently supplied hashes for every executable:

```sh
python3 tools/skia/canvaskit.py build \
  --profile canvaskit-production \
  --source-dir /absolute/path/to/skia \
  --emsdk-dir /absolute/path/to/emsdk \
  --build-dir /absolute/path/to/canvaskit-out \
  --toolchain-id emscripten-4.0.7-linux-x86_64 \
  --gn-sha256 "$PINNED_GN_SHA256" \
  --ninja-sha256 "$PINNED_NINJA_SHA256" \
  --emcc-sha256 "$PINNED_EMCC_SHA256" \
  --emxx-sha256 "$PINNED_EMXX_SHA256" \
  --emar-sha256 "$PINNED_EMAR_SHA256"
```

Use `--profile canvaskit-software-qualification` for the CPU-only build. Its
receipt pins `skia_enable_ganesh=false`, WebGL/WebGPU/Dawn/Graphite off, and the
same paragraph, ICU, font, codec, and memory contract as the production lane.
A receipt from one profile cannot be packaged as the other.

## Installed artifact layout

An unpacked native artifact has one stable root suitable for
`FISSION_SKIA_ARTIFACT_DIR`:

```text
manifest.json
include/fission_skia.h
lib/fission_skia_bridge.a       # .lib on Windows MSVC
lib/<manifest-declared libs>
licenses/<component>.txt
metadata/build.json
```

`manifest.json` contains the exact source revision, bridge ABI, target, profile,
features, compiler/runtime/deployment contract, ordered static link libraries,
system libraries, Apple frameworks, and a SHA-256 plus byte length for every
payload file. Its native link section always has this shape:

```json
{
  "link_search_paths": ["lib"],
  "static_libraries": [
    "fission_skia_bridge",
    "svg",
    "skparagraph",
    "skshaper",
    "skunicode",
    "skia"
  ],
  "system_libraries": [],
  "frameworks": []
}
```

The actual list is target/profile-specific and must describe every supplied
library. For a profile with target recipes, packaging and verification require
the exact system libraries and frameworks declared for that target; callers
cannot weaken or expand the contract through link metadata. No build script
guesses system libraries or frameworks.

CanvasKit uses the same pin and profile identity but a Web-specific layout:

```text
manifest.json
web/canvaskit.js
web/canvaskit.wasm
web/fission_skia_web.js
web/fission_skia_commands.js
web/fission_skia_executor.js
web/fission_skia_paragraph_wire.js
web/fission_skia_paragraph_unicode.js
web/fission_skia_paragraph.js
licenses/<component>.txt
```

Every runtime module is a named, hashed manifest asset. Packaging requires the
complete fixed bridge set from one directory so an executor, protocol decoder,
or paragraph dependency cannot be omitted accidentally. Test fixtures and
`package.json` are not shipped.
CanvasKit's WOFF2 support is part of its FreeType build and uses Brotli; those
two notices cover the code actually linked, so there is no fabricated
standalone WOFF2 notice input.

## Packaging and verification

Package a completed native build with explicit input metadata:

```sh
python3 tools/skia/skia.py package-native \
  --profile native-raster \
  --target x86_64-unknown-linux-gnu \
  --fission-version 0.11.0 \
  --build-metadata /absolute/path/to/out/fission-skia-build.json \
  --bridge-header crates/rendering/fission-skia-sys/include/fission_skia.h \
  --library fission_skia_bridge=/absolute/path/to/libfission_skia_bridge.a \
  --library skia=/absolute/path/to/libskia.a \
  --library skparagraph=/absolute/path/to/libskparagraph.a \
  --library skshaper=/absolute/path/to/libskshaper.a \
  --library skunicode=/absolute/path/to/libskunicode.a \
  --library svg=/absolute/path/to/libsvg.a \
  --link-metadata /absolute/path/to/link.json \
  --deployment-metadata /absolute/path/to/deployment.json \
  --license skia=/absolute/path/to/skia/LICENSE \
  --license fission=/absolute/path/to/fission/LICENSE \
  --license expat=/absolute/path/to/expat/COPYING \
  --license freetype=/absolute/path/to/freetype/docs/FTL.TXT \
  --license harfbuzz=/absolute/path/to/harfbuzz/COPYING \
  --license icu=/absolute/path/to/icu/LICENSE \
  --license libjpeg-turbo=/absolute/path/to/libjpeg-turbo/LICENSE.md \
  --license libpng=/absolute/path/to/libpng/LICENSE \
  --license libwebp=/absolute/path/to/libwebp/COPYING \
  --license wuffs=/absolute/path/to/wuffs/LICENSE \
  --license zlib=/absolute/path/to/zlib/LICENSE \
  --output /absolute/path/to/staged-artifact \
  --archive /absolute/path/to/fission-skia.tar.gz \
  --source-date-epoch 1786406400
```

Deployment metadata contains exactly `toolchain` and `deployment`. Required
deployment keys come from the selected target:

```json
{
  "toolchain": {
    "id": "clang-20-linux-x86_64",
    "compiler": "clang 20.1.8",
    "runtime_abi": "libstdc++ / C++20 ABI"
  },
  "deployment": {
    "libc": "glibc",
    "libc_version": "2.31",
    "cxx_runtime": "libstdc++.so.6"
  }
}
```

The selected profile and target recipe declare the exact required licence,
upstream-library, system-library, and framework sets; missing and extra inputs
all fail. Add `cpu-features` only for Android and the Vulkan notices only for a
Vulkan Ganesh recipe. The supplied upstream libraries must match the completed
build receipt byte for byte.

Package a completed CanvasKit build by supplying its two receipted outputs, the
Fission-owned runtime module directory, exact deployment metadata, and all
profile licences:

```sh
python3 tools/skia/canvaskit.py package \
  --profile canvaskit-production \
  --target wasm32-unknown-unknown \
  --fission-version 0.11.0 \
  --build-metadata /absolute/path/to/canvaskit-out/fission-canvaskit-build.json \
  --canvaskit-js /absolute/path/to/canvaskit-out/canvaskit.js \
  --canvaskit-wasm /absolute/path/to/canvaskit-out/canvaskit.wasm \
  --bridge-dir crates/rendering/fission-skia-sys/web \
  --deployment-metadata /absolute/path/to/canvaskit-deployment.json \
  --license fission=/absolute/path/to/fission/LICENSE \
  --license skia=/absolute/path/to/skia/LICENSE \
  --license brotli=/absolute/path/to/brotli/LICENSE \
  --license freetype=/absolute/path/to/freetype/docs/FTL.TXT \
  --license harfbuzz=/absolute/path/to/harfbuzz/COPYING \
  --license icu=/absolute/path/to/icu/LICENSE \
  --license libjpeg-turbo=/absolute/path/to/libjpeg-turbo/LICENSE.md \
  --license libpng=/absolute/path/to/libpng/LICENSE \
  --license libwebp=/absolute/path/to/libwebp/COPYING \
  --license wuffs=/absolute/path/to/wuffs/LICENSE \
  --license zlib=/absolute/path/to/zlib/LICENSE \
  --output /absolute/path/to/staged-canvaskit \
  --archive /absolute/path/to/fission-canvaskit.tar.gz \
  --source-date-epoch 1786406400
```

The software profile uses the same packaging command with its own build receipt
and `--profile canvaskit-software-qualification`. Both outputs remain
`qualified: false`; the word “qualification” identifies the comparison lane,
not evidence that qualification has run or passed.

Archives require a numeric `SOURCE_DATE_EPOCH`, use normalized metadata, and
produce a sibling `.sha256` file. Both files are staged privately and published
without overwriting existing paths; a publication failure rolls back the other
member of the pair. Verification requires an independently known archive
digest. It copies and hashes the input once, then inspects and extracts only that private
snapshot. Links, special files, unsafe paths, collisions, and undeclared files
are rejected before the artifact is accepted:

```sh
python3 tools/skia/skia.py verify \
  --archive /absolute/path/to/fission-skia.tar.gz \
  --sha256 0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef \
  --profile native-raster \
  --target x86_64-unknown-linux-gnu
```

For an unpacked local/vendor artifact, set `FISSION_SKIA_ARTIFACT_DIR` to its
root and verify that directory. `FISSION_SKIA_PROFILE` must exactly equal its
manifest profile. `FISSION_SKIA_LINK_LIBS` is reserved as an explicit local
link override; release artifacts derive their complete link contract from the
manifest.

These foundation packages are always emitted as `origin: local-build` and
`qualified: false`; local Git and vendored source receipts also carry
`qualified: false`. Digests bind the inputs and outputs together but do not
authenticate who built them.

`promote.py` supplies the fail-closed release transition. Promotion requires a
fully passing report for the complete frozen backend matrix, checks that the
requested target/profile cell names the exact packaged artifact, re-runs the
strict package verifier, and changes only `origin` and `qualified`. The input
archive and promoted output must be different files:

```sh
python3 tools/skia/promote.py promote \
  --kind native \
  --archive /absolute/path/to/unqualified.tar.gz \
  --sha256 0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef \
  --profile native-raster \
  --target x86_64-unknown-linux-gnu \
  --qualification-report /absolute/path/to/qualification-report.json \
  --evidence /absolute/path/to/run-linux-skia.json \
  --evidence /absolute/path/to/every-other-required-run.json \
  --qualification-target-id linux-x86_64-gnu \
  --qualification-profile-id skia-only \
  --source-date-epoch 1786406400 \
  --output /absolute/path/to/fission-skia-qualified.tar.gz
```

Repeat `--evidence` for every target/profile cell. Promotion recomputes the
report through the authoritative qualification tool and requires byte-for-byte
equality with `--qualification-report`; report booleans supplied by a caller are
never trusted. The selected cell must bind both the artifact ID and the exact
unqualified archive SHA-256. The deterministic promoted archive differs only
because its two release-owned manifest fields record that completed transition.

The release workflow must then publish and attest that exact promoted archive.
Only after publication may it enter the bundled artifact lock. `lock` snapshots
and verifies the archive once, requires GitHub's SLSA v1 attestation from the
dedicated Fission artifact workflow at the exact source commit, rejects
self-hosted signers, requires a trusted timestamp, and accepts only the
canonical immutable GitHub release-asset URL:

```sh
python3 tools/skia/promote.py lock \
  --kind native \
  --archive /absolute/path/to/fission-skia-qualified.tar.gz \
  --sha256 fedcba9876543210fedcba9876543210fedcba9876543210fedcba9876543210 \
  --profile native-raster \
  --target x86_64-unknown-linux-gnu \
  --source-digest 0123456789abcdef0123456789abcdef01234567 \
  --url https://github.com/fission-ui/fission/releases/download/skia-0.10.1/fission-skia-qualified.tar.gz
```

The lock remains empty until real artifacts pass the frozen matrix; the
promotion command does not waive missing evidence. The crate resolver uses the
bundled lock as its release trust decision and re-verifies the archive,
manifest, identity, ABI, target, profile, and complete payload before linking.

## GitHub artifact workflow

`.github/workflows/skia-artifacts.yml` is the only trusted Skia artifact signer.
Its manual operations are deliberately separate and resumable:

- `build` derives the current release set from the frozen qualification matrix,
  builds native raster and Ganesh artifacts plus the production and software
  CanvasKit profiles, verifies every archive, attests its exact bytes, and
  rejects a partial matrix. These artifacts remain explicitly unqualified and
  are useful for local integration and qualification work.
- `qualify` first requires every reviewed environment, input, build, toolchain,
  artifact digest, and numeric budget in the frozen manifest. It then requires
  a checked-in real platform collector at
  `tools/backend-qualification/collect.py`; it never synthesizes results. The
  repository does not yet contain that collector, so this operation currently
  fails closed by design.
- `promote` accepts only distinct successful same-commit build and qualification
  run IDs, an existing release tag at that exact commit, the complete raw
  evidence set, and the production environment gate. It re-verifies build
  provenance, recomputes the complete report, promotes and attests exact bytes,
  refuses to replace a non-identical release asset, verifies release
  provenance, and opens a review containing only the generated lock update.

Real execution also requires reviewed repository variables containing the
expected SHA-256 values for the GN, Ninja, and Emscripten executables selected
by each hosted-runner family, together with their stable toolchain IDs. Missing
or malformed values stop before a tool is executed. Build provenance cannot
substitute for product qualification, and neither operation writes the bundled
artifact lock directly to `main`.

Run the script tests without Cargo:

```sh
python3 -m unittest discover -s tools/skia/tests -v
```
