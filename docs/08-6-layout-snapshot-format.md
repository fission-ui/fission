# 8.6 Layout Snapshot Format

This section defines the **Layout Snapshot**: the immutable, canonical data structure produced by the layout pass.
The snapshot is the sole contract between layout and downstream systems such as rendering, hit testing, semantics geometry, and tests.

If geometry is not present in the snapshot, it does not exist.

---

## 8.6.1 Purpose of the Layout Snapshot

The layout snapshot exists to:

- capture all computed geometry deterministically,
- decouple layout from rendering and input,
- enable headless inspection and testing,
- support diffing, caching, and replay.

The snapshot is a value, not a live structure.

---

## 8.6.2 Snapshot Immutability

Layout snapshots are immutable.

Rules:
- produced atomically by the layout pass,
- never mutated in place,
- replaced wholesale on rebuild.

Immutability enables:
- safe sharing across systems,
- snapshot diffing,
- deterministic caching.

---

## 8.6.3 Snapshot Addressing Model

All snapshot data is keyed by **NodeId**.

Properties:
- NodeId identity is stable and deterministic,
- all geometry references NodeIds, never pointers,
- missing NodeIds are errors.

There is no implicit tree traversal API.

---

## 8.6.4 Core Snapshot Structure

Conceptually, a layout snapshot contains:

- metadata (version, rounding policy, viewport),
- a NodeId-indexed geometry table,
- optional debug and provenance data.

Example (conceptual):

```rust
LayoutSnapshot {
    version,
    viewport,
    rounding_policy,
    nodes: HashMap<NodeId, LayoutNode>,
}
```

The actual storage may be optimized but is semantically equivalent.

---

## 8.6.5 Per-Node Geometry Record

Each `LayoutNode` record includes:

- layout rect (local and global),
- baseline (optional),
- paint bounds,
- clip bounds,
- hit-test bounds,
- scroll-related transforms.

All fields are explicit.

---

## 8.6.6 Coordinate Representation

All geometry is stored in:

- logical units (pre-rounding),
- rounded pixel units (post-rounding).

Rules:
- both representations may be present,
- rounding is applied consistently,
- tests may assert either form explicitly.

No floating-point drift is permitted.

---

## 8.6.7 Visibility and Clipping State

Visibility is derived, not stored.

Rules:
- clip bounds define visibility,
- layout rects always exist even if clipped,
- empty clip bounds imply non-visible nodes.

Derived visibility is deterministic.

---

## 8.6.8 Scroll State Representation

For scroll containers, the snapshot includes:

- content size,
- viewport rect,
- scroll offset,
- scroll extents.

Scroll state is explicit and inspectable.

---

## 8.6.9 Snapshot Versioning

The snapshot format is versioned.

Rules:
- version changes are explicit,
- backward compatibility is maintained where possible,
- incompatible changes require explicit migration.

Versioning ensures long-lived tests remain valid.

---

## 8.6.10 Snapshot Diffing

Snapshots are designed for diffing.

Diffs may compare:
- geometry changes,
- node additions/removals,
- clip or paint bound changes.

Diffs are structural, not visual heuristics.

---

## 8.6.11 Testing Against Snapshots

Tests interact directly with the snapshot.

Examples:

```rust
let rect = find("button").layout_rect();
assert_eq!(rect.width(), 80);
```

Snapshot-based tests are:
- fast,
- deterministic,
- renderer-independent.

---

## 8.6.12 Instrumentation and Debug Metadata

Optional metadata may be attached:

- provenance (source widgets),
- constraint resolution traces,
- intermediate values.

Metadata is non-semantic and stripped in production builds if desired.

---

## 8.6.13 Serialization and Persistence

Layout snapshots are serializable.

Uses:
- golden layout tests,
- regression debugging,
- offline analysis.

Serialization format is stable and versioned.

---

## 8.6.14 Summary

The layout snapshot:

- is the authoritative geometry record,
- decouples layout from rendering and input,
- enables testing, diffing, and tooling,
- preserves determinism across platforms.

It is the boundary where geometry becomes data.

---
