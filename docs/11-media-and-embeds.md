# Media and Embeds Spec (v1)

This document defines the **v1 media and embeds model** for the framework.
It consolidates design and integration requirements for:
- images,
- video,
- audio,
- “embed” surfaces (e.g., WebView, 3D, Lottie).

The system is designed for:
- **determinism** (identical results given identical inputs),
- **headless execution** (CI-friendly),
- **testability** (precise assertions and replay),
- **cross-platform shells** (desktop/mobile/web).

This spec assumes the authoring and lowering model defined in:
- `Widget<S>::build(...) -> impl IntoWidget<S>`,
- automatic lowering via `Lower`,
- optional custom lowering via `Node::Custom(CustomNode)` + `LowerDyn`.

---

## 1. Goals and Non-Goals

### Goals
- A single, coherent contract for embedding “external” media/renderers into the UI tree.
- Deterministic sizing, layout, composition, input routing, and semantics.
- Clear separation of responsibilities:
  - Core owns: layout integration, determinism, clock, test harness, snapshots.
  - Shell owns: platform-specific decoding/surfaces, OS integration.
- Support both:
  - **pure-core** rendering (images decoded into pixels, rasterized deterministically),
  - **surface embeds** (video/3D/webview) where the platform provides a surface.

### Non-Goals (v1)
- Full feature parity with platform-native players (DRM, PIP, advanced buffering control).
- Arbitrary OS-managed view embedding that bypasses core compositing.
- Perfectly identical video decode results across all hardware (see determinism policy below).

---

## 2. Definitions

### 2.1 Media vs Embed
- **Media**: assets that ultimately produce pixels or sound (image/video/audio).
- **Embed**: any component whose internal rendering/runtime is not the framework’s normal paint model (video surface, webview, 3D engine, Lottie runtime).

v1 treats both under a shared “embed contract.”

### 2.2 Two Rendering Modes
1. **Raster Mode (pure core)**  
   The framework produces pixels via deterministic decode + paint (best for images, some Lottie cases).
2. **Surface Mode (platform-backed)**  
   The shell provides a renderable surface or texture, and the framework composes it (needed for video/3D/webview).

---

## 3. Architectural Contract

### 3.1 Identity
All media/embeds must have stable identity:

- `WidgetNodeId` (stable in snapshots and tests)
- optional `AssetId` (stable identifier for assets)
- optional `EmbedInstanceId` (runtime instance identifier, stable across frames for the same node id)

### 3.2 Explicit State Ownership
- Persistent app state: user-owned (`AppState`).
- Playback state: **runtime-owned** (`RuntimeState`) keyed by node id, unless explicitly bound to app state.
- Time: **framework-owned clock** (owned clock; see animation doc).

### 3.3 Lowering: preferred patterns
Media/embeds may be implemented via:

- **Primitive nodes** (e.g. `Image` as a primitive that lowers to paint ops), or
- **`Node::Custom`** lowerers (`LowerDyn`) for advanced or platform-specific behavior.

v1 recommends:
- **Image**: primitive (pure core path first).
- **Video/Audio/WebView/3D/Lottie**: embed via `Node::Custom` or an explicit `LayoutOp::Embed` (see §4).

---

## 4. Core IR Model: `LayoutOp::Embed` (Recommended)

To make testing, instrumentation, and accessibility integration direct, embeds should be **first-class** at the Core IR boundary.

### 4.1 Embed layout op
Introduce an op:

```rust
LayoutOp::Embed {
    id: WidgetNodeId,
    kind: EmbedKind,
    constraints: EmbedConstraints,
    child: Option<NodeId>, // optional overlay subtree (future)
}
```

Where:
- `EmbedKind` describes the embed type: `Video`, `Audio`, `Web`, `Lottie`, `ThreeD`, `Custom("...")`
- `EmbedConstraints` includes intrinsic sizing rules and aspect ratio (if relevant)

### 4.2 Output metrics
Layout must produce:
- `viewport_rect` (where the embed appears)
- `content_rect` (if different)
- `clip_rect` (if clipped)
- intrinsic size / aspect ratio info (for deterministic layout)

These appear in layout snapshots and are queryable in tests.

### 4.3 Paint compilation
Embeds compile into paint instructions as either:

- **Raster content** (image frames rendered as normal paint ops), or
- **Surface layers** (a “texture/layer” paint op referencing a shell-provided surface)

For surface mode:

```text
PushClipRect(viewport)
DrawSurface(surface_id, viewport)
PopClipRect
```

---

## 5. Shell Contract (Platform Responsibilities)

### 5.1 Resource provisioning
The shell provides services for:
- asset loading (bytes) with explicit ids/hashes
- decoding (image/video) when in surface mode
- creating and managing surfaces/textures
- audio playback backend

### 5.2 Determinism policy for platform decoding
- **Images (v1):** determinism is required. Use pinned decoder settings and stable pixel output.
- **Video (v1):** deterministic *timing and state* is required; pixel-identical frames may not be guaranteed across hardware.
  - Tests should prefer mocked or rasterized test backends for video when pixel determinism is required.
- **Audio (v1):** deterministic playback state and time are required; waveform output is not validated by pixel tests.

### 5.3 Explicit versioning
Shell must expose:
- backend identifiers
- codec/decoder version identifiers (when relevant)
to include in snapshots for debugging and reproducibility.

---

## 6. Runtime State Model

### 6.1 Media runtime state
Runtime stores per-node media state (examples):

```rust
pub struct MediaState {
    pub status: MediaStatus,       // stopped/playing/paused/buffering/error
    pub position_ms: u64,
    pub duration_ms: Option<u64>,
    pub volume: f32,
    pub rate: f32,
    pub looped: bool,
}
```

### 6.2 Control surfaces and actions
Interaction with media is performed via:
- typed Actions (e.g. `Play`, `Pause`, `SeekTo`)
- semantics actions (accessibility)

Media widgets should not store closures; they store action envelopes or use runtime-controlled defaults.

### 6.3 Owned clock integration
Playback progression uses the framework-owned clock:
- advancing time advances playback position deterministically
- pause freezes progression
- seek sets position deterministically

---

## 7. Input, Hit Testing, and Focus

### 7.1 Hit testing
Embeds participate in hit testing with deterministic rules:
- treat embed viewport as hit region by default
- allow custom hit shapes via custom lowering (`Node::Custom`) if needed

### 7.2 Input routing
- Pointer/keyboard events inside the embed viewport route to:
  - the embed (e.g. webview),
  - or to overlay UI if overlays exist (future).
Routing rules must be explicit and stable.

### 7.3 Focus handling
Embeds can be focusable:
- expose semantics role and focusability
- focus order is deterministic and integrated into the traversal rules

---

## 8. Semantics and Accessibility

Media/embeds must expose semantics nodes:
- role:
  - `Image`
  - `Video`
  - `Audio`
  - `Document/Web`
  - `Canvas/3D`
- label/description (from authoring or i18n)
- actions:
  - play/pause/seek/volume (media)
  - activate/open (web)
  - custom actions where appropriate

Platform bridges translate these to native APIs.

---

## 9. Testing and CI

### 9.1 Required test APIs
The harness must support:
- querying embed rects (`rect_in_viewport`, `paint_bounds`)
- querying media state (`position_ms`, `status`)
- controlling playback deterministically:
  - `play(id)`, `pause(id)`, `seek(id, ms)`
  - `advance_time(dt)` to advance playback
- mocking backends:
  - deterministic raster backend for images
  - mock video backend (returns deterministic test frames)
  - mock audio backend (state-only or deterministic waveform)

### 9.2 Golden image policy
- Image widgets: suitable for golden tests if decode is deterministic.
- Video: golden tests should use mock frames or a pinned software decoder in CI.
- WebView/3D: prefer structural/geometry/semantics tests unless using a deterministic software renderer.

---

## 10. Snapshot Format (v1)

Snapshots must capture enough information to replay and debug:

### 10.1 Embed identity and layout
- node id
- embed kind
- viewport rect / clip rect
- intrinsic size/aspect ratio
- paint map spans (for debug)

### 10.2 Media state
Per embed:
- status
- playback position
- duration (if known)
- rate/volume/looping
- current asset id/hash
- backend identifiers (decoder versions where relevant)

### 10.3 Clock state
- current runtime time (owned clock)
- any active animation/media timers

---

## 11. Implementation Guidance (v1)

### 11.1 Images
- implement as a primitive lowering to paint ops:
  - decode image bytes to pixels deterministically
  - cache by `AssetId`/hash
  - compute intrinsic size deterministically
- tests can assert:
  - intrinsic size
  - raster output (goldens)

### 11.2 Video
- implement as an embed:
  - surface mode in production
  - mock/raster backend in CI
- core owns:
  - playback state machine
  - time/progress
  - layout rect and composition layer
- shell owns:
  - decoding and surface provisioning

### 11.3 Audio
- implement as an embed with no paint by default:
  - semantics and controls still exist
  - runtime state still exists
- shell owns playback; core owns determinism of time/state transitions.

### 11.4 Lottie
- recommended: embed
- two modes:
  - raster mode (deterministic) for CI and golden testing
  - optional optimized platform mode, but must still obey owned clock and explicit state

---

## 12. Summary

v1 media/embeds are defined by a strict contract:
- stable identity
- explicit runtime state
- owned clock integration
- first-class IR representation (`LayoutOp::Embed` recommended)
- deterministic testing support via mocks/raster paths

This enables:
- real-world production support across platforms,
- deterministic CI and replay,
- clear boundaries between core and shell.
