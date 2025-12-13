# 10.4 Scroll State in Snapshots

This section defines how scroll state is represented in **layout and render snapshots**.
Scroll state must be fully observable, serializable, and replayable to support determinism, testing, and tooling.

If scroll state is not in the snapshot, it does not exist.

---

## 10.4.1 Snapshot Visibility of Scroll State

Scroll state is captured explicitly in snapshots.

Snapshots expose:
- current scroll offset per axis,
- scroll extents (min/max),
- viewport rect,
- content size,
- active scroll physics model (by identifier).

No scroll-relevant data is implicit or hidden.

---

## 10.4.2 Layout Snapshot Representation

In the layout snapshot, scroll containers include:

- `viewport_rect`
- `content_rect`
- `scroll_offset`
- `scroll_extent`
- `clip_bounds` derived from viewport

These fields are stored alongside other per-node geometry.

---

## 10.4.3 Render / Display Snapshot Representation

In render-facing snapshots:

- scroll offsets are already applied as transforms,
- clip ops reflect viewport clipping,
- original scroll state remains available as metadata.

Renderers do not need to infer scroll state.

---

## 10.4.4 Coordinate Space Accounting

Snapshots preserve multiple coordinate spaces:

- content-local space (pre-scroll),
- viewport space (post-scroll),
- global space.

Scroll state provides the explicit transform between these spaces.

---

## 10.4.5 Snapshot Determinism Guarantees

Scroll state in snapshots is deterministic because:

- offsets come from explicit reducer state,
- extents come from deterministic layout,
- no runtime or platform data is consulted.

Identical state and inputs yield identical snapshot scroll data.

---

## 10.4.6 Snapshot Diffing and Regression Testing

Scroll-aware snapshot diffing can detect:

- offset changes,
- extent changes due to layout regressions,
- viewport size changes,
- physics configuration changes.

This enables precise regression tests for scrolling behavior.

---

## 10.4.7 Serialization and Persistence

Scroll state is serialized as part of snapshots.

Uses include:
- golden layout tests,
- replaying interaction traces,
- offline debugging.

Serialization format is stable and versioned.

---

## 10.4.8 Instrumentation and Inspection

Inspection APIs may query:

- current scroll offsets,
- visible content ranges,
- clipped vs non-clipped nodes,
- scroll container ancestry.

Instrumentation reads snapshot data only.

---

## 10.4.9 Accessibility Implications

Accessibility tooling relies on snapshot scroll state to:

- announce current position,
- determine visibility,
- compute reading order,
- trigger scroll actions programmatically.

Consistency across platforms is guaranteed.

---

## 10.4.10 Error Conditions

Snapshot-related scroll errors include:

- offsets outside extents,
- missing viewport data,
- inconsistent content sizes.

Such errors are detected during snapshot construction.

---

## 10.4.11 Summary

Scroll state in snapshots:

- makes scrolling fully observable,
- enables deterministic testing and replay,
- decouples scroll logic from rendering,
- supports accessibility and tooling.

Snapshots are the authoritative record of scroll reality.

---
