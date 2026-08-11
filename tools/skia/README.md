# Fission Skia artifact foundation

This directory owns the reproducible-input and artifact-layout contract for the
direct Fission Skia integration. It does not contain or advertise a production
artifact yet.

The current direct bridge contract is ABI v9. ABI changes are explicit artifact
identity changes; the tooling will not package or verify a header from another
bridge ABI.

The source pin is the immutable upstream commit
`cf5c36972b73698eb3939cda147ea47152670312`. The upstream
`canvaskit/0.41.0` tag and milestone 148 are recorded only as useful review
hints. They are not selection inputs and the pin is explicitly unqualified.

`artifacts.lock.json` is intentionally empty. Adding an entry is a release
operation that must happen only after the corresponding archive, provenance,
and qualification evidence exist.

## Supported vocabulary

`config.json` declares these profiles:

- `native-raster`, whose foundation build recipe is available;
- `native-ganesh`, available for Vulkan on Linux GNU x86_64 and arm64;
- `native-graphite-qualification`, planned and never an implicit fallback;
- `canvaskit-production`, planned WebGL plus raster fallback;
- `canvaskit-software-qualification`, planned CPU-only Web qualification.

Linux GNU and musl, macOS, Windows MSVC, Android ABIs, iOS device/simulator
slices, and interactive Web target names are declared. Declared means the tools
recognize the identity; it does not mean an artifact exists or is qualified.
Profiles without an implemented recipe fail rather than producing a plausible
but incomplete archive. `native-ganesh` classifies every declared native
target: Linux musl is explicitly unsupported until its C++/fontconfig
toolchain is reproducible, while macOS, Windows, Android, and iOS remain
explicitly pending their platform surface and presentation contracts.

The first Ganesh slice enables Vulkan and VMA while disabling GL, X11, Metal,
Direct3D, Dawn, and Graphite. Fission's Vulkan surface bridge is the WSI owner
for this profile; Xlib, XCB, and Wayland are declared presentation routes and
do not require Skia's GLX integration. `skia_use_x11=false` therefore removes a
Skia GLX dependency without narrowing that planned WSI set. The exact native
consumer link contract is `dl`, `fontconfig`, and the Vulkan loader. Raster
`SkSurface` remains in the same artifact as the fallback. The Ganesh recipe
compiles the two Vulkan bridge units and defines
`FISSION_SKIA_ENABLE_GANESH_VULKAN=1`; the raster recipe names only the common
bridge units and carries no profile define.

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

Select the Linux Vulkan build by changing `--profile` to `native-ganesh`.
Both source and prebuilt consumers select it with
`FISSION_SKIA_PROFILE=native-ganesh`; omitting the variable continues to select
`native-raster`.

`FISSION_SKIA_SOURCE_DIR` and `FISSION_SKIA_BUILD_DIR` are the equivalent
explicit environment overrides. If a command-line value and environment value
conflict, the command fails. A Git checkout must be clean and exactly at the
pin. A source vendor directory without `.git` must contain
`FISSION_SKIA_SOURCE_REVISION` whose only line is the exact pinned commit.

Android and iOS recipes require their target-specific GN inputs explicitly,
for example `--gn-arg ndk=/absolute/path --gn-arg ndk_api=26`. Extra arguments
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
defines. The Fission bridge is built from the bridge source owned by
`fission-skia-sys` and supplied to the packaging step; packaging checks its
header ABI and binds the header, library digest, sources, and defines into the
artifact manifest. A raster bridge receipt therefore cannot be relabelled as a
Ganesh artifact.

The native profiles deliberately ship one stable notice bundle across their
supported targets. Their common required components are Fission, Skia, Expat,
FreeType, HarfBuzz, ICU, libjpeg-turbo, libpng, libwebp, Wuffs, zlib, and
Android's conditional cpu-features dependency. `native-ganesh` additionally
includes the Vulkan headers and Vulkan Memory Allocator notices. Expat and the
other explicitly non-system libraries are built from the pinned Skia
dependency checkout; target-conditional notices remain in every artifact so a
profile's contract does not vary by host.

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
`web/canvaskit.js`, `web/canvaskit.wasm`, and the Fission Web bridge. The layout
is pinned now; its build is deliberately rejected until the batched bridge,
font/resource policy, destruction rules, SVG lowering, and browser lifecycle
are implemented.

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
  --license cpu-features=/absolute/path/to/cpu-features/LICENSE \
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

The selected profile declares the exact required licence and upstream-library
sets; missing and extra inputs both fail. The supplied upstream libraries must
match the completed build receipt byte for byte.

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
authenticate who built them. Release promotion, signatures, trusted provenance,
and population of `artifacts.lock.json` remain fail-closed release-pipeline
work; this tool does not imply they happened.

Run the script tests without Cargo:

```sh
python3 -m unittest discover -s tools/skia/tests -v
```
