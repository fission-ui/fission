# 17.1 Incremental Updates and Diffing

This section explains how the framework achieves **incremental updates** through precise,
deterministic **diffing** across structure, layout, paint, and semantics.
Incrementality is a performance feature derived from correctness guarantees, not a heuristic.

Nothing updates unless the data proves it must.

---

## 17.1.1 Goals

Incremental updates and diffing must:

- minimize recomputation work,
- preserve deterministic outcomes,
- produce explainable invalidation,
- avoid heuristic dependency tracking,
- scale to large UI trees.

Incrementality is explicit and auditable.

---

## 17.1.2 The Snapshot-Centric Model

All incrementality is snapshot-driven.

Key snapshots include:
- structural snapshot,
- layout snapshot,
- display list snapshot,
- semantics snapshot.

Each snapshot is immutable once produced.
Diffs are computed between snapshots, not during mutation.

---

## 17.1.3 Structural Diffing as the Root Signal

Structural diffs are the primary invalidation signal.

Structural changes include:
- node addition or removal,
- reparenting,
- reordering,
- semantic or action binding changes.

If structure is unchanged, many downstream stages can be skipped.

---

## 17.1.4 Stable Identity Enables Incrementality

Stable node identity is mandatory.

Identity sources:
- explicit author keys,
- deterministic structural hashing,
- versioned fallback rules.

Identity stability allows reuse of:
- layout results,
- paint commands,
- semantics mappings.

Without stable identity, incrementality collapses.

---

## 17.1.5 Layout Incrementality

Layout invalidation is precise.

Rules:
- only nodes whose constraints or inputs changed are re-laid out,
- unaffected subtrees reuse previous layout results,
- intrinsic size changes propagate upward deterministically.

Layout never re-evaluates the full tree unnecessarily.

---

## 17.1.6 Paint and Display List Diffing

Display lists are diffed structurally.

Diff granularity includes:
- command insertion/removal,
- parameter changes,
- transform updates.

Unchanged spans are reused directly.
PaintMap spans preserve node-to-command correspondence.

---

## 17.1.7 Semantics Incrementality

Semantics updates are scoped.

Rules:
- semantic changes propagate only when semantic data changes,
- accessibility trees update incrementally,
- focus order diffs are explicit.

Accessibility performance scales with UI size.

---

## 17.1.8 Action and State Diffing

State changes are localized.

Rules:
- reducers update specific state paths,
- subscriptions declare explicit dependencies,
- only affected nodes re-evaluate bindings.

There is no implicit dependency graph.

---

## 17.1.9 Diff Composition Across Stages

Diffs compose across stages:

- structural diff → layout diff
- layout diff → display list diff
- semantics diff → accessibility update

Each stage consumes diffs from the previous stage.

---

## 17.1.10 Determinism of Diffs

Diff computation is deterministic because:

- snapshots are canonical,
- ordering rules are fixed,
- identity rules are stable.

Identical inputs always produce identical diffs.

---

## 17.1.11 Debuggability of Incremental Behavior

Incremental behavior is inspectable.

Tools can:
- visualize invalidation regions,
- explain why a node was recomputed,
- show diff graphs across stages.

There are no hidden optimizations.

---

## 17.1.12 Performance Characteristics

Incremental diffing provides:

- near-O(changed_nodes) behavior,
- bounded memory overhead,
- predictable worst-case costs.

Full recomputation remains a valid fallback.

---

## 17.1.13 Failure Modes and Safeguards

Safeguards include:

- versioned snapshot formats,
- identity collision detection,
- fallback to full recompute on inconsistency.

Correctness is never compromised for speed.

---

## 17.1.14 Summary

Incremental updates work because:

- UI state is explicit and immutable,
- identity is stable,
- diffs are first-class data,
- optimization is driven by proof, not guesswork.

Performance emerges naturally from a correct model.
