# Diagnostics and DevTools Protocol Spec (v1)

This document defines the **debug/insights story** for Fission v1:
- structured diagnostics events,
- snapshot formats for inspection and tests,
- enablement (compile-time and runtime),
- optional transport/protocol for future IDE tooling (VS Code / IntelliJ) and a dedicated DevTools UI.

This spec is designed to preserve the rich debug work already added while ensuring:
- near-zero overhead when disabled,
- deterministic, queryable outputs when enabled.

---

## 1. Goals and Non-Goals

### Goals
- Keep existing debug visibility without shipping verbose logs by default.
- Provide **structured**, stable diagnostics (not ad-hoc printf).
- Enable deterministic reproduction and tooling:
  - snapshots,
  - event streams,
  - node selection + provenance.
- Support multiple sinks:
  - stdout/file (v1),
  - IPC/WebSocket (optional v1),
  - in-process API for tests.

### Non-Goals (v1)
- A full-featured DevTools UI (we only define the contract).
- Deep performance profiling across threads (v2).
- Remote device debugging over WAN (v2).

---

## 2. Build Modes and Enablement

Diagnostics must be cheap when off. Implement a two-layer switch:

### 2.1 Compile-time gating
- Feature flag: `fission_diagnostics`
  - When disabled, diagnostic emission is compiled out (except fatal errors).
- Debug assertions remain behind `cfg!(debug_assertions)`.

Recommended build profiles:
- **release**: `fission_diagnostics` off by default
- **profile**: `fission_diagnostics` on, but runtime-disabled unless enabled
- **debug**: `fission_diagnostics` on, runtime-enabled by default for selected categories

### 2.2 Runtime enablement
Provide:
- env vars for shells
- builder API for embedding

Env vars (desktop shell):
- `FISSION_DIAG=layout,diff,paint` (comma-separated categories; `*` for all)
- `FISSION_DIAG_LEVEL=error|warn|info|debug|trace`
- `FISSION_DIAG_SINK=stdout|file:<path>|ipc:<addr>`
- `FISSION_DIAG_SAMPLING=1.0` (0..1)
- `FISSION_DIAG_FRAME_FILTER=every:1|every:10|range:100-200`

Builder API (all shells):
```rust
app.with_diagnostics(DiagnosticsConfig { ... })
```

---

## 3. Structured Events Model

Diagnostics are emitted as structured events (schema-first).

### 3.1 Terminology
- **Frame**: one logical update+render iteration (driven by owned clock / event loop).
- **StableId**: stable node identifier (WidgetNodeId).
- **Span**: a scoped operation (layout pass, paint compilation).
- **Event**: an instantaneous record (diff summary, invariant violation).

### 3.2 Categories
v1 categories:
- `frame`
- `diff`
- `layout`
- `paint`
- `raster`
- `input`
- `semantics`
- `animation`
- `media`
- `invariants`
- `test`

### 3.3 Severity levels
`error`, `warn`, `info`, `debug`, `trace`.

### 3.4 Event envelope
All events share a common envelope:

```rust
pub struct DiagEvent {
    pub schema_version: u16,        // v1 = 1
    pub timestamp_ns: u64,          // from owned clock (deterministic)
    pub frame_no: u64,
    pub category: DiagCategory,
    pub level: DiagLevel,
    pub event: DiagEventKind,       // typed payload
}
```

Important:
- Timestamps come from the owned clock, not OS time, to keep determinism in tests.

### 3.5 Typed payloads (core set)
Minimum v1 event kinds:

- `FrameStart { root: StableId }`
- `FrameEnd { stats: FrameStats }`

- `DiffSummary {
    nodes_total: u32,
    nodes_created: u32,
    nodes_removed: u32,
    nodes_changed: u32,
    dirty_layout: u32,
    dirty_paint: u32,
    reason_counts: Vec<(DirtyReason, u32)>,
  }`

- `LayoutSummary {
    reused: u32,
    recomputed: u32,
    boundaries_hit: u32,
    cycles_detected: u32,
  }`

- `PaintSummary {
    segments_reused: u32,
    segments_regenerated: u32,
    paint_ops_total: u32,
  }`

- `RasterSummary {
    cache_hits: u32,
    cache_misses: u32,
    tiles_rasterized: u32,
  }`

- `AnimationSummary {
    active_count: u32,
    started: u32,
    replaced: u32,
    ended: u32,
  }`

- `MediaSummary {
    video_nodes: u32,
    audio_nodes: u32,
    embeds_total: u32,
  }`

- `InvariantViolation {
    kind: InvariantKind,
    node: Option<StableId>,
    details: String, // compact; avoid giant dumps by default
    dump_ref: Option<String>, // points to a snapshot file or side channel
  }`

- `InputEvent {
    kind: InputKind,
    target: Option<StableId>,
    position: Option<(f32,f32)>,
  }`

---

## 4. Snapshots

Snapshots are deterministic, structured representations of runtime state.

### 4.1 Snapshot types
v1 snapshot types:
- `CoreIrSnapshot`
- `LayoutSnapshot`
- `PaintSnapshot`
- `SemanticsSnapshot`
- `RuntimeStateSnapshot` (scroll offsets, focus, animation states, media states)
- `CompositeSnapshot` (bundle references to all above)

### 4.2 Snapshot API
Runtime must provide:

```rust
pub trait SnapshotProvider {
    fn snapshot(&self, kind: SnapshotKind) -> SnapshotBlob;
}
```

Where `SnapshotBlob` is JSON in v1:
- UTF-8 JSON with schema version
- stable id references
- stable ordering everywhere (sort by StableId)

### 4.3 Determinism requirements
- Stable ordering of nodes and children.
- Rounded/quantized numeric fields according to determinism rules.
- Include pinned dependency versions:
  - font bundle version
  - theme version id
  - i18n bundle version/locale
  - backend identifiers (renderer/media)

### 4.4 Snapshot triggers
- On-demand via DevTools protocol (see §6)
- On invariant violation (write minimal graph dump)
- In tests:
  - always available for assertions

---

## 5. Provenance (Source Mapping)

To support IDE tooling and human debugging, nodes should carry optional provenance:

```rust
pub struct Provenance {
    pub file: Option<String>,
    pub line: Option<u32>,
    pub column: Option<u32>,
    pub widget_type: Option<String>,
    pub debug_tag: Option<String>,
}
```

Rules:
- Provenance is optional and should be compiled out or stripped in release if desired.
- When present, it must be stable and deterministic.
- Custom nodes should include a debug tag and ideally widget type.

---

## 6. DevTools Transport and Protocol (Optional v1)

v1 defines a minimal protocol that can be implemented later without changing the runtime.

### 6.1 Transport options
- `stdout`/`file` sink: always supported (v1 minimum)
- `ipc` sink (optional):
  - WebSocket on localhost (recommended)
  - or stdio JSON-RPC for editor extensions

### 6.2 Protocol versioning
All protocol messages include:
- `protocol_version` (v1 = 1)
- `request_id` for request/response matching

### 6.3 Requests (minimum)
- `EnableDiagnostics { categories, level }`
- `DisableDiagnostics`
- `GetStats`
- `GetSnapshot { kind }`
- `FindNode { selector }`
- `GetNodeInfo { id }`
- `SetOverlay { enabled, mode }` (optional: draw bounds, paint order, semantics)

### 6.4 Responses
- `Ok { request_id, payload }`
- `Error { request_id, code, message }`
- `Event { event: DiagEvent }` (server push when subscribed)

### 6.5 Selectors
Selectors should align with the test selector model:
- by stable id
- by debug tag
- by semantics role/label
- by provenance (file:line)
- by “hit test at point” (requires layout snapshot)

---

## 7. Sinks and Overhead Control

### 7.1 Sinks
Implement sinks as pluggable consumers:

- `StdoutSink`
- `FileSink`
- `RingBufferSink` (recommended for always-on errors without spam)
- `IpcSink` (optional)

### 7.2 Overhead controls
- Category filtering
- Level filtering
- Sampling and frame filtering
- Ring buffer for recent events
- Avoid allocating large strings when disabled:
  - use `tracing`-style macros or lazily format strings

### 7.3 Zero-cost when disabled
When diagnostics are disabled:
- event emission macros should compile to no-ops
- no allocation, no hashing, no string formatting

---

## 8. Integration Points in the Runtime

Emit events at these boundaries:

- Frame loop:
  - `FrameStart`, `FrameEnd`
- Diff:
  - `DiffSummary`
- Layout:
  - `LayoutSummary`
  - `InvariantViolation` on validation failures
- Paint compilation:
  - `PaintSummary`
- Raster:
  - `RasterSummary`
- Animation:
  - `AnimationSummary`
- Media:
  - `MediaSummary`
- Input and semantics:
  - `InputEvent`, `SemanticsActionDispatched`

All events must reference `StableId` wherever possible.

---

## 9. Tests and CI

### 9.1 Deterministic logs
In tests, diagnostics should be capturable:
- harness can assert on event sequences
- snapshots can be dumped on failure

### 9.2 Strict modes
Use env toggles:
- `FISSION_LAYOUT_STRICT=1` (panic on invariant violations)
- `FISSION_ALLOW_FULL_REBUILD=1` (diagnostic escape hatch only)
- `FISSION_DIAG=...` to capture details in CI artifacts

---

## 10. Minimal v1 Implementation Plan

1. Create `fission-diagnostics` crate:
   - event types + schema
   - sinks (stdout/file/ring-buffer)
   - config and filtering
2. Wire runtime emission points:
   - frame/diff/layout/paint/animation/media/invariants
3. Implement snapshot provider for:
   - layout snapshot
   - semantics snapshot
   - runtime state snapshot
4. Add provenance fields (optional; behind feature flag)
5. (Optional) Implement localhost WebSocket for DevTools protocol.

---

## 11. Summary

Fission v1 diagnostics are:
- **structured** (schema-first),
- **deterministic** (owned clock + stable ordering),
- **cheap when off** (compile-time + runtime gating),
- and designed to power future tooling (VS Code/IntelliJ) via stable IDs, selectors, and snapshots.
