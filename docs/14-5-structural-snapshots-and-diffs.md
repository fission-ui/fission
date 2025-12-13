# 14.5 Structural Snapshots and Diffs

This section defines **structural snapshots** and **structural diffs**.
Structural snapshots capture the *shape and meaning* of the UI—independent of rendering—while diffs provide precise, explainable comparisons between states.

Structural diffs answer *what changed* and *why*, not just *that something changed*.

---

## 14.5.1 Goals

Structural snapshots and diffs must:

- be deterministic and replay-safe,
- represent UI meaning, not incidental implementation,
- be stable across rendering backends,
- support precise regression detection,
- produce explainable, minimal diffs.

They are the primary artifact for non-visual correctness.

---

## 14.5.2 What Is a Structural Snapshot

A structural snapshot is a canonical representation of:

- the node tree (hierarchy and ordering),
- stable node identities and keys,
- semantic roles and labels,
- action bindings,
- state references (by path, not value),
- focus and accessibility order.

It intentionally excludes pixels and GPU artifacts.

---

## 14.5.3 Snapshot Canonicalization

Structural snapshots are canonicalized.

Rules:
- children are ordered deterministically,
- optional fields are normalized,
- elided defaults are removed,
- identifiers are stable and versioned.

Canonicalization ensures byte-for-byte stability.

---

## 14.5.4 Node Identity and Stability

Each node in a snapshot has a stable identity.

Sources of identity include:
- explicit author keys,
- derived structural hashes,
- versioned fallback rules.

Identity stability is independent of memory addresses.

---

## 14.5.5 Included vs Excluded Data

Included:
- node type / role,
- key or identity,
- parent/child relationships,
- semantics and actions,
- enabled/disabled state.

Excluded:
- layout geometry (covered elsewhere),
- paint details,
- backend-specific handles.

Separation keeps diffs focused.

---

## 14.5.6 Structural Diff Model

A structural diff compares two snapshots and produces:

- additions (nodes introduced),
- removals (nodes deleted),
- moves (reparenting or reordering),
- modifications (attribute or semantic changes).

Each change is localized and typed.

---

## 14.5.7 Diff Granularity

Diffs operate at multiple granularities:

- tree-level (large refactors),
- subtree-level (component changes),
- node-level (attribute updates).

Granularity is configurable but deterministic.

---

## 14.5.8 Ordering and Move Detection

Move detection is first-class.

Rules:
- reordering siblings is detected as a move, not delete+add,
- keyed nodes preserve identity across moves,
- unkeyed nodes follow canonical matching rules.

This avoids noisy diffs.

---

## 14.5.9 Semantic-Aware Diffs

Diffs are semantics-aware.

Examples:
- role change is distinct from label change,
- action binding change is highlighted explicitly,
- accessibility order changes are surfaced.

Diffs reflect user-visible meaning.

---

## 14.5.10 Snapshot Diff Output

Diff output is structured.

Formats may include:
- machine-readable JSON,
- human-readable summaries,
- tree-annotated views.

Output is stable for CI and tooling.

---

## 14.5.11 Regression Testing With Diffs

Structural diffs power regression tests.

Tests may assert:
- no structural changes,
- specific expected changes,
- absence of breaking changes.

This enables safe refactors.

---

## 14.5.12 Performance Characteristics

Structural diffing is efficient.

Properties:
- linear in snapshot size,
- incremental where possible,
- bounded memory usage.

Large UIs remain diffable in CI.

---

## 14.5.13 Error Handling

Structural diff errors include:

- incompatible snapshot versions,
- invalid identity collisions,
- malformed canonical forms.

Errors are explicit and actionable.

---

## 14.5.14 Summary

Structural snapshots and diffs provide:

- a stable representation of UI meaning,
- precise and explainable change detection,
- backend-independent correctness checks,
- confidence during refactors and evolution.

They are the backbone of non-visual UI verification.

---
