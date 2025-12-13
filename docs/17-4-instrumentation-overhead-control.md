# 17.4 Instrumentation Overhead Control

This section explains how the framework provides **deep instrumentation** for testing, debugging,
and observability while ensuring **zero or near-zero overhead** in production builds.
Instrumentation is a capability that can be enabled, scoped, and removed without altering semantics.

Observability must never compromise performance correctness.

---

## 17.4.1 Design Goals

Instrumentation must:

- be fully optional,
- impose zero cost when disabled,
- have predictable and bounded cost when enabled,
- never change observable behavior,
- be composable and scopeable.

Instrumentation is orthogonal to UI semantics.

---

## 17.4.2 Compile-Time Feature Gating

The primary control mechanism is compile-time feature flags.

Examples:
- `instrumentation`
- `snapshots`
- `action-tracing`
- `debug-metadata`

Rules:
- disabled features compile to no-ops,
- dead code is eliminated by the compiler,
- hot paths contain no conditional branches when disabled.

Production builds pay nothing for unused features.

---

## 17.4.3 Tiered Instrumentation Levels

Instrumentation is tiered.

Typical levels:
1. **None** – production default
2. **Light** – metrics and counters only
3. **Structural** – snapshots and diffs
4. **Full** – geometry, paint maps, traces

Each tier has explicit cost characteristics.

---

## 17.4.4 Scoped Instrumentation

Instrumentation can be scoped temporally and spatially.

Examples:
- enable instrumentation for a single frame,
- instrument a specific subtree,
- capture traces around a failing test only.

Scoping prevents global performance impact.

---

## 17.4.5 Data Capture Strategy

Instrumentation captures data passively.

Rules:
- no recomputation for instrumentation,
- data is tapped from existing pipelines,
- captured data is immutable once recorded.

Instrumentation never drives execution.

---

## 17.4.6 Arena Separation

Instrumentation data uses separate arenas.

Benefits:
- clear lifetime boundaries,
- easy reclamation,
- no fragmentation of hot-path arenas.

Production arenas remain untouched.

---

## 17.4.7 Snapshot Cost Management

Snapshot overhead is controlled via:

- structural sharing,
- selective field inclusion,
- bounded history windows,
- explicit snapshot triggers.

Snapshots are not free—but their cost is explicit.

---

## 17.4.8 Action Tracing Overhead

Action tracing is lightweight.

Properties:
- append-only buffers,
- fixed-size records,
- optional sampling.

Tracing cost is linear in number of actions.

---

## 17.4.9 Geometry and Paint Instrumentation

Geometry-heavy instrumentation is isolated.

Rules:
- geometry queries are snapshot-based,
- paint maps are optional,
- raster-level inspection is opt-in.

Production rendering paths are unaffected.

---

## 17.4.10 Runtime Enablement and Disablement

Some instrumentation can be toggled at runtime.

Rules:
- toggles affect future frames only,
- no retroactive capture,
- state remains consistent.

Runtime toggles never introduce branches in hot loops.

---

## 17.4.11 Failure Diagnostics Without Always-On Cost

On failure:

- instrumentation may be enabled automatically,
- execution may be replayed deterministically,
- data is captured post hoc.

This avoids paying costs during healthy operation.

---

## 17.4.12 Measuring Instrumentation Overhead

Instrumentation overhead is measurable.

Metrics include:
- additional allocations,
- time per stage,
- memory retained per snapshot.

This enables informed trade-offs.

---

## 17.4.13 Safety and Correctness

Instrumentation must never:

- mutate Core state,
- influence scheduling decisions,
- alter timing semantics,
- reorder operations.

Violations are treated as critical bugs.

---

## 17.4.14 Production Use Cases

In production, instrumentation supports:

- sampling-based telemetry,
- targeted debugging builds,
- user-reported issue reproduction.

Full instrumentation is rarely required.

---

## 17.4.15 Summary

Instrumentation overhead is controlled because:

- it is designed in from the start,
- it is compile-time gated,
- it is data-oriented and passive,
- it is fully removable.

The system is observable when needed—and invisible when not.
