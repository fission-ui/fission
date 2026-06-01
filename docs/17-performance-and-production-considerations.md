# Incremental Updates and Retained Caches Spec (v1)

This document defines how the runtime achieves high performance while preserving:
- deterministic behavior,
- headless execution,
- testability and replay,
- cross-platform consistency.

The core strategy is **hybrid retained mode**:
- the authoring tree (`Widget::build -> impl IntoWidget`) may be rebuilt frequently,
- **compiled artifacts are retained** and updated incrementally.

Retained artifacts include:
- canonical Core IR,
- layout results,
- display list segments,
- optional raster caches.

---

## 1. Goals and Non-Goals

### Goals
- Reclaim performance under animation and interaction-heavy workloads.
- Avoid full-tree layout/paint/raster for small changes.
- Preserve determinism and stable snapshots.
- Provide clear invalidation rules that are easy to implement and debug.
- Make incremental behavior testable (e.g., assert which nodes were recomputed).

### Non-Goals (v1)
- Perfect optimality (no need for the minimum possible recomputation).
- Sophisticated multi-thread scheduling (may come later).
- Automatic partial rebuild of authoring widgets (authoring may still rebuild each frame).

---

## 2. Retained Layers (Analogy to Flutter)

Fission retains compiled artifacts roughly analogous to Flutter’s retained layers:
- **Canonical Core IR** ≈ RenderObject graph representation
- **Layout Cache** ≈ layout state
- **Display List Cache** ≈ layer tree / composited paint commands
- **Raster Cache (optional)** ≈ cached layers/pictures/tiles

The authoring `Node` tree is an input that may be regenerated; retained artifacts are the performance layer.

---

## 3. Canonical Core IR as the Incremental Boundary

### 3.1 Why Core IR is the boundary
Incremental updates are anchored at **canonical Core IR** because it is:
- closed-world (small op set),
- stable-ordered,
- deterministic and serializable,
- suitable for hashing and diffing,
- independent of authoring ergonomics.

### 3.2 Required invariants
To enable incremental updates, Core IR must satisfy:
- stable traversal order,
- stable node identity (see §4),
- canonicalization (elision rules, normalized rounding),
- no non-deterministic iteration.

---

## 4. Identity and Hashing

Incremental reuse requires two concepts:
- **Identity**: “this node corresponds to the same logical element as before”
- **Equivalence**: “this node’s behavior/output is unchanged”

### 4.1 Stable identities
Every node has:
- `StableId` (derived from `WidgetNodeId` / keys / structural identity)
- `NodeId` (arena-local id; changes per build)

`StableId` is the cross-frame anchor.
`NodeId` is per-frame storage.

### 4.2 Content hashes
Each Core IR node must have a stable **content hash** (aka structural hash):

```
hash(node) = H(op_tag, normalized_fields, child_stable_ids, op_version)
```

Rules:
- hash uses **normalized** values (post-canonicalization rounding).
- child references are stable ids, not arena pointers.
- for unordered collections: sort deterministically before hashing.

### 4.3 Dependency hashes
In addition to content hash, some nodes depend on external pinned resources:
- font bundle version
- theme version id (or resolved tokens)
- i18n bundle version + locale
- device pixel ratio / env inputs

In v1, treat these as part of the `FrameEnvHash`.

---

## 5. Diff Algorithm (Core IR)

Given:
- previous frame: `PrevCore { root_stable_id, arena, hashes }`
- next frame: `NextCore { root_stable_id, arena, hashes }`

The diff produces:
- `DirtyStructural`: nodes whose structure or parameters changed
- `DirtyLayout`: nodes requiring layout recomputation
- `DirtyPaint`: nodes requiring repaint/display list regeneration

### 5.1 Basic rule
For each `StableId` present in both frames:
- if `content_hash` unchanged → structurally identical
- else → structurally dirty

For ids present only in next → created
For ids present only in prev → removed

### 5.2 Dirty propagation rules
Dirty flags propagate upward or downward depending on subsystem:

- **Layout dirtiness** propagates upward when child size may affect parent.
- **Paint dirtiness** propagates upward when child paint bounds affect parent clipping/compositing, otherwise may be localized.

The runtime should implement conservative propagation for v1:
- if a node is structurally dirty → mark it dirty for both layout and paint.
- if a node is layout dirty → mark ancestors layout dirty until reaching a boundary (see §6.3).
- if a node is paint dirty → mark ancestors paint dirty if they depend on child paint bounds (clip/opacity/layer).

---

## 6. Layout Cache

### 6.1 Cache key
Layout results are cached per node per constraints signature:

```
LayoutKey = (StableId, ConstraintsSig)
```

`ConstraintsSig` must be deterministic and include:
- min/max width/height (normalized)
- axis constraints (for scroll/flex)
- environment factors that affect measurement (DPR, font scale if applicable)

### 6.2 Cached outputs
Store:
- measured size
- baselines
- paint bounds and clip bounds (post-layout)
- child placements (offsets)
- any layout-specific metadata (scroll extents, intrinsic sizes)

### 6.3 Layout boundaries (stop propagation)
Some nodes form layout boundaries, limiting upward propagation:
- fixed-size boxes
- positioned/absolute nodes
- scroll viewports (viewport size fixed by parent constraints)
- explicit constraints widgets

For v1, implement boundaries gradually; a conservative approach is acceptable but slower.

### 6.4 Reuse rule
A node’s layout can be reused if:
- node is not layout-dirty, AND
- same `ConstraintsSig`, AND
- any external measurement dependencies are unchanged (fonts, locale for text shaping).

---

## 7. Paint / Display List Cache

### 7.1 Segment model
Each node produces a **paint segment** (or a small set of spans) in the display list.
The display list for the frame is assembled by concatenating segments in deterministic paint order.

### 7.2 Cache key
```
PaintKey = (StableId, PaintInputsHash)
```

`PaintInputsHash` includes:
- node content hash (or a paint-specific hash)
- computed layout outputs that affect paint (size, offsets, clip rects)
- resolved theme tokens used for painting
- resolved text shaping results (or a shaping hash)

### 7.3 Cached outputs
- display list commands (segment)
- paint bounds
- optional “layer hints” (opacity, saveLayer needs)

### 7.4 Reuse rule
A node’s paint segment can be reused if:
- node is not paint-dirty, AND
- `PaintInputsHash` unchanged.

### 7.5 PaintMap integration
Maintain `PaintMap: StableId -> (start_index, end_index)` mapping to:
- locate segments for inspection/debug,
- support test queries like `paint_bounds()` deterministically.

---

## 8. Optional Raster Cache (Recommended for Animations)

Animations often change transforms/opacity while subtree contents remain constant.
Raster cache can drastically reduce cost.

### 8.1 What to cache
Cache rasterized output of subtrees that are:
- expensive to paint (text-heavy, complex paths),
- moved/transformed frequently,
- clipped to a stable rect.

### 8.2 Cache key
```
RasterKey = (StableId, SubtreePaintHash, RasterScaleSig)
```

- `SubtreePaintHash` must exclude transient transform/opacity if those are applied outside the cached surface.
- `RasterScaleSig` includes DPR and scale factors.

### 8.3 Invalidation
Invalidate if:
- subtree paint hash changes,
- clip rect changes,
- DPR/scale changes,
- any child producing pixels changes.

### 8.4 Determinism
Raster cache must be an optimization only.
Cache hits/misses must not change visible output.
If non-deterministic GPU behavior exists, use software raster in headless CI.

---

## 9. Participation of `Node::Custom` / `LowerDyn`

Custom lowered nodes must integrate with incremental updates.

### 9.1 Required: stable key
`LowerDyn::stable_key()` must return a deterministic value derived from:
- stable id,
- relevant parameters,
- referenced assets/theme tokens.

This key is used to form the node’s content hash (or to seed it).

### 9.2 Dependency reporting (recommended)
For better invalidation, custom nodes should optionally report:
- whether they affect layout based on child size,
- whether they produce paint bounds dependent on runtime state,
- which env/theme keys they use.

v1 can be conservative:
- treat custom nodes as layout+paint dirty whenever stable_key changes.

---

## 10. Animation Integration

### 10.1 What changes per frame
Under animation:
- transforms/opacity may change frequently
- layout often does NOT change
- subtree paint commands may NOT change

### 10.2 Rules
- If only transform/opacity changes:
  - skip layout
  - reuse paint segments
  - optionally reuse raster cache
- If animation changes layout-affecting properties (size constraints):
  - mark layout dirty
  - propagate appropriately

### 10.3 Owned clock
Animation evaluation uses owned clock and produces deterministic property values.
These values feed into hashing:
- transform/opacity are part of parent paint hash, not subtree paint hash (to preserve cacheability).

---

## 11. Scheduling and Update Pipeline (Per Frame)

1. **Build**: `Widget::build -> impl IntoWidget` (authoring)
2. **Lower + Canonicalize**: `Node -> CoreIR` (stable ids + hashes)
3. **Diff**: PrevCore vs NextCore → dirty sets
4. **Layout**:
   - reuse cached layout where allowed
   - recompute dirty nodes with constraint propagation
5. **Paint Compilation**:
   - reuse cached segments where allowed
   - regenerate dirty segments
6. **Raster/Present**:
   - optionally raster cache
   - submit display list to renderer

All steps must be deterministic given identical inputs.

---

## 12. Instrumentation and Debugging

The runtime should expose diagnostics:
- number of nodes rebuilt/lowered
- number of nodes laid out vs reused
- number of paint segments regenerated vs reused
- raster cache hit/miss counts
- dirty reasons per node (structural, constraints changed, env changed)

These are essential for performance tuning and for making LLM-driven changes safe.

---

## 13. Minimal v1 Implementation Plan

Implement in order:

1. Stable ids + canonical hashes for Core IR nodes.
2. Basic Core IR diff producing dirty sets.
3. Layout cache keyed by (StableId, ConstraintsSig).
4. Paint cache keyed by (StableId, PaintInputsHash) and display list segmentation per node.
5. Conservative propagation rules (optimize later).
6. Optional raster cache for transform/opacity animations.

This will reclaim performance for animations and restore usability while preserving determinism.

---
