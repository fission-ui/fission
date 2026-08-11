# 13.5 Future Renderer Integration

This section describes how **future renderer backends** integrate with the 3D
and 2D systems without changing Core semantics. Renderer integration is
designed to be extensible, replaceable, and verifiable through Fission-owned
contracts.

Renderers are consumers of data, not owners of meaning.

---

## 13.5.1 Goals for Renderer Integration

Future renderer integration must:

- preserve Core determinism and semantics,
- require no application-widget changes,
- compose typed 3D and other external producers with the 2D frame,
- allow incremental backend development,
- enable shared semantic conformance and backend-specific visual testing.

Renderer innovation must never leak into Core logic.

---

## 13.5.2 Renderer as a Semantic Consumer

Production renderer integrations must consume immutable frames without owning
UI meaning. Interactive graphics sessions are deliberately stateful: they own
surfaces, derived resources, caches, presentation, and recovery.

Required rules for a production backend profile:
- renderers read, never mutate, state,
- renderers do not own time,
- renderers do not schedule frames,
- unsupported operations and dropped frames produce explicit diagnostics.

A renderer crash must not corrupt Core state.

---

## 13.5.3 Fission-Owned Renderer Input Contracts

Renderer inputs are owned by Fission rather than by a backend dependency. The
new cross-crate contracts are intentionally internal and remain evolvable while
Phase 1 exercises them through the current implementations; this RFC does not
declare their Rust spelling or serialization stable yet.

Inputs include:
- display lists (2D),
- render descriptions (3D),
- resolved geometry and transforms,
- explicit clip and blend states,
- immutable resource snapshots,
- typed external-surface bindings,
- viewport, damage, lifecycle, and presentation inputs.

Application and widget compatibility is a framework guarantee. Internal frame,
resource, and backend contracts may evolve without exposing a dependency type
or changing application code.

---

## 13.5.4 Multi-Renderer Support

Multiple renderers may coexist.

Examples:
- Skia-based renderer,
- the current Vello-centered renderer,
- pure Rust software renderer,
- WebGPU renderer,
- platform-native compositors.

Renderer selection is a configuration concern.

---

## 13.5.5 Renderer Capability Negotiation

Renderers advertise capabilities explicitly.

Examples:
- supported blend modes,
- texture formats,
- shader features.

Rules:
- Core behavior does not change based on capabilities,
- unsupported features fail explicitly with provenance,
- capability negotiation is explicit and testable.

---

## 13.5.6 3D Backend and Renderer Separation

3D producers and 2D graphics backends are distinct.

Target rules:
- neutral 3D models are consumed by an optional general-GPU implementation,
- the resulting work enters the ordered frame through an external-surface binding,
- either side may be swapped independently.

The current bridge is transitional: it identifies a tagged neutral 3D payload,
derives placement from `DrawSurface`, and uses a `DirectTarget` binding while
the wgpu renderer still encodes into the active target. Typed interchange
images and producer synchronization are later work.

This separation prevents lock-in.

---

## 13.5.7 Headless and Software Renderers

Software renderers are first-class.

Uses include:
- CI and golden testing,
- debugging and instrumentation,
- platforms without GPU access.

Software renderers must satisfy shared semantics and maintain their own visual
goldens; they are not required to rasterize identically to another engine.

---

## 13.5.8 Conformance and Validation

Production renderer qualification requires:

- snapshot comparison,
- render-description diffing,
- backend-specific golden raster tests,
- shared semantic and non-text geometric conformance tests.

Semantic non-conformance or a regression against a backend's approved visual
baseline is a bug. A cross-backend pixel difference is not automatically one.

---

## 13.5.9 Performance Boundaries

Performance optimizations are local to renderers.

Rules:
- batching, caching, and reordering must preserve output,
- precision changes must be explicit,
- performance heuristics must not affect semantics.

Correctness precedes speed.

---

## 13.5.10 Error Handling and Fallbacks

Renderer failures are isolated.

Required rules:
- failures produce explicit error states,
- fallbacks are deterministic,
- Core continues to function without renderer recovery logic.

The target contracts make renderer implementations replaceable without making
their lifecycle or recovery responsibilities stateless.

---

## 13.5.11 Long-Term Evolution

The integration model supports future evolution:

- new GPU APIs,
- hybrid ray/path tracing,
- remote or streamed rendering,
- LLM-assisted rendering analysis.

All evolution must remain behind Fission-owned contracts. Those contracts earn
stability through production implementations and conformance evidence rather
than being frozen at the start of the refactor.

---

## 13.5.12 Summary

Future renderer integration works because:

- renderers consume semantic frames without owning UI meaning,
- contracts are explicit and owned by Fission,
- semantics live in the Core,
- conformance must be enforced by qualification tests.

Renderers may change freely; the meaning of the UI does not.

---
