# 13.5 Future Renderer Integration

This section describes how **future renderer backends** integrate with the 3D (and 2D) systems without changing Core semantics.
Renderer integration is explicitly designed to be extensible, replaceable, and verifiable against reference behavior.

Renderers are consumers of data, not owners of meaning.

---

## 13.5.1 Goals for Renderer Integration

Future renderer integration must:

- preserve Core determinism and semantics,
- require no changes to Core IR or snapshots,
- support both 2D and 3D uniformly,
- allow incremental backend development,
- enable conformance testing against reference outputs.

Renderer innovation must never leak into Core logic.

---

## 13.5.2 Renderer as a Pure Consumer

Renderers consume completed snapshots.

Rules:
- renderers read, never mutate, state,
- renderers do not own time,
- renderers do not schedule frames,
- renderers may drop frames without semantic impact.

A renderer crash must not corrupt Core state.

---

## 13.5.3 Stable Renderer Input Contracts

Renderer inputs are stable and versioned.

Inputs include:
- display lists (2D),
- render descriptions (3D),
- resolved geometry and transforms,
- explicit clip and blend states,
- resource handles resolved by the platform shell.

Backward compatibility is a Core guarantee.

---

## 13.5.4 Multi-Renderer Support

Multiple renderers may coexist.

Examples:
- Skia-based renderer (initial),
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
- unsupported features degrade deterministically,
- capability negotiation is explicit and testable.

---

## 13.5.6 3D Backend and Renderer Separation

3D backends and renderers are distinct.

Rules:
- 3D backends evaluate scenes and produce descriptions,
- renderers turn descriptions into GPU commands,
- either side may be swapped independently.

This separation prevents lock-in.

---

## 13.5.7 Headless and Software Renderers

Software renderers are first-class.

Uses include:
- CI and golden testing,
- debugging and instrumentation,
- platforms without GPU access.

Software renderers must match reference semantics.

---

## 13.5.8 Conformance and Validation

All renderers are validated via:

- snapshot comparison,
- render-description diffing,
- golden raster tests (where applicable),
- cross-backend equivalence tests.

Non-conformance is a bug.

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

Rules:
- failures produce explicit error states,
- fallbacks are deterministic,
- Core continues to function without renderer recovery logic.

Renderers are replaceable components.

---

## 13.5.11 Long-Term Evolution

The integration model supports future evolution:

- new GPU APIs,
- hybrid ray/path tracing,
- remote or streamed rendering,
- LLM-assisted rendering analysis.

All evolution happens behind stable contracts.

---

## 13.5.12 Summary

Future renderer integration works because:

- renderers are pure consumers,
- contracts are explicit and versioned,
- semantics live in the Core,
- conformance is enforced by tests.

Renderers may change freely; the meaning of the UI does not.

---
