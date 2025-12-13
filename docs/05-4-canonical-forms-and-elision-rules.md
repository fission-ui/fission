# 5.4 Canonical Forms and Elision Rules

This section defines the canonical forms enforced on Core IR and the elision rules used to remove redundant or semantically neutral structures.
Canonical forms ensure that equivalent UI semantics always produce identical Core IR representations.

Canonicalization is semantic normalization, not optimization.

---

## 5.4.1 Purpose of Canonical Forms

Canonical forms exist to:

- make semantic equivalence structurally comparable,
- stabilize snapshot diffs,
- eliminate authoring and lowering variability,
- support deterministic testing and replay.

Without canonical forms, small refactors would cause large, noisy diffs.

---

## 5.4.2 Canonicalization as a Required Phase

Canonicalization is mandatory.

Rules:
- all Core IR must pass canonicalization before layout or rendering,
- canonicalization is versioned and deterministic,
- canonicalization preserves semantics exactly.

Skipping canonicalization is a correctness error.

---

## 5.4.3 Structural Canonical Forms

Structural canonicalization enforces:

- flattening of nested transparent groups,
- removal of empty groups,
- elimination of fragments,
- explicit grouping boundaries where required.

Example:

```text
Group
 └─ Group
     └─ Child
```

Canonical form:

```text
Group
 └─ Child
```

---

## 5.4.4 Identity Preservation Rules

Canonicalization must preserve identity.

Rules:
- nodes with identities are never merged,
- identity-bearing nodes are not reordered,
- identity is preserved unless the node is removed entirely.

Elision is only permitted for identity-neutral nodes.

---

## 5.4.5 Default Elision and Normalization

Defaults are handled explicitly.

Rules:
- default-valued fields may be elided *or* made explicit, but consistently,
- canonical form chooses one representation per version,
- absence and default are never ambiguous.

This prevents representation drift.

---

## 5.4.6 Ordering Canonicalization

Ordering is canonicalized where semantics allow.

Examples:
- action lists are ordered deterministically,
- semantics attributes are sorted by stable keys,
- paint ops are ordered explicitly.

Ordering that affects semantics is never altered.

---

## 5.4.7 Redundant Wrapper Elision

Wrappers that do not affect semantics may be elided.

Examples:
- layout containers with identity-neutral defaults,
- paint groups with no effects,
- semantics wrappers with no declared attributes.

Wrappers that affect identity, ordering, or scope are never elided.

---

## 5.4.8 Canonicalization of Keys and Scopes

Keyed and scoped nodes obey stricter rules:

- keyed nodes are never reordered across keys,
- scopes are never removed,
- scope boundaries are preserved even if empty.

Keys and scopes are semantic boundaries.

---

## 5.4.9 Elision Safety Rules

Elision is permitted only if:

- semantics are unchanged,
- identity is not affected,
- traversal order is preserved,
- observability via tests is unchanged.

If any doubt exists, elision is forbidden.

---

## 5.4.10 Canonicalization and Versioning

Canonicalization rules are versioned.

Implications:
- older Core IR snapshots retain their canonical form,
- new rules do not retroactively change old semantics,
- diffs across versions are explainable.

Canonicalization is part of the Core IR contract.

---

## 5.4.11 Testing Canonical Forms

Canonicalization is tested via:

- golden Core IR snapshots,
- round-trip canonicalization tests,
- equivalence tests across authoring variations.

Tests assert canonical structure, not authoring structure.

---

## 5.4.12 Summary

Canonical forms and elision rules:

- enforce a unique representation for each semantic meaning,
- eliminate structural noise,
- preserve identity and observability,
- underpin reliable diffing and testing.

They are essential to the framework’s determinism guarantees.

---
