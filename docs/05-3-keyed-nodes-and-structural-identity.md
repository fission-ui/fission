# 5.3 Keyed Nodes and Structural Identity

This section defines how *keyed nodes* interact with the deterministic identity system and how structural identity is preserved across rebuilds.
Keying is an explicit mechanism for expressing identity intent when structural position alone is insufficient.

Keyed identity is opt-in, explicit, and narrowly scoped.

---

## 5.3.1 The Problem Structural Identity Alone Cannot Solve

Structural identity derives Node IDs from tree position.

This works well when:
- sibling order is stable,
- insertions/removals are rare,
- nodes are not reordered dynamically.

However, in dynamic collections (lists, grids, virtualized views):
- insertion shifts indices,
- reordering breaks identity continuity,
- unrelated nodes appear to change identity.

Keyed nodes exist to solve this problem explicitly.

---

## 5.3.2 Definition of a Keyed Node

A keyed node is a Core IR node that declares an explicit **key** in addition to its structural position.

Conceptually:

```rust
Key {
    value: KeyValue,
    child: Node,
}
```

The key:
- is stable application data,
- participates in identity derivation,
- scopes identity within its parent.

Keys are not global identifiers.

---

## 5.3.3 Key vs Explicit ID

Keys are distinct from explicit IDs.

| Explicit ID | Key |
|------------|-----|
| Global within a scope | Local to a parent |
| Selected directly by tests | Indirectly stabilizes identity |
| Rare, opt-in | Common for collections |
| Author-facing | Structure-facing |

Keys influence identity derivation but do not replace Node IDs.

---

## 5.3.4 Identity Derivation With Keys

When a key is present:

- identity derivation prefers the key over positional index,
- sibling ordering still matters for traversal,
- identity remains stable across reordering.

Example:

```text
List
 ├─ Key("a") → Item
 ├─ Key("b") → Item
 └─ Key("c") → Item
```

Reordering items does not change their identities as long as keys remain the same.

---

## 5.3.5 Scope of Keys

Keys are scoped to their immediate parent.

Rules:
- keys must be unique among siblings,
- duplicate keys are a lowering error,
- keys have no meaning outside their scope.

This prevents accidental global coupling.

---

## 5.3.6 Interaction With Canonicalization

Canonicalization respects keyed identity.

Rules:
- keyed nodes are never reordered across key boundaries,
- canonicalization must not collapse keyed nodes,
- structural equivalence considers keys explicitly.

Keys are part of the semantic identity.

---

## 5.3.7 When to Use Keys

Keys should be used when:

- rendering dynamic lists,
- reordering children intentionally,
- inserting/removing items frequently,
- identity continuity matters for animation or state.

Keys should *not* be used:
- indiscriminately,
- as a substitute for explicit IDs,
- for static structure.

---

## 5.3.8 Relationship to State and Animation

Keyed identity enables:

- stable animation targets across reorders,
- preserved scroll positions,
- state continuity for list items.

Without keys, these behaviors degrade predictably but undesirably.

---

## 5.3.9 Test Implications

Keyed identity improves test stability:

- selectors remain valid across reordering,
- diffs reflect semantic changes, not index churn,
- replay is deterministic.

Tests may still select by role or label rather than keys.

---

## 5.3.10 Error Handling

Lowering validates keyed nodes for:

- duplicate keys among siblings,
- unsupported key types,
- missing child nodes.

Errors are deterministic and reported with structural context.

---

## 5.3.11 Summary

Keyed nodes provide:

- explicit control over identity stability,
- predictable behavior in dynamic structures,
- minimal intrusion into the identity system.

They complement structural identity without weakening determinism.

---
