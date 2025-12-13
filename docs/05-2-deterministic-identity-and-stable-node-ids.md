# 5.2 Deterministic Identity and Stable Node IDs

This section defines the identity model used during lowering and explains how stable, deterministic Node IDs are assigned and preserved.
Identity is foundational to layout caching, event routing, accessibility, testing, and snapshot diffs.

Identity is data, not an implementation artifact.

---

## 5.2.1 Why Identity Must Be Deterministic

Node identity underpins the entire runtime:

- layout results are keyed by identity,
- paint bounds and hit regions are associated with identity,
- actions target identities,
- tests select nodes by identity-derived selectors.

If identity is nondeterministic:
- caches become invalid,
- diffs become noisy,
- tests become flaky.

Deterministic identity is therefore a hard requirement.

---

## 5.2.2 Identity vs Memory Addresses

Node identity must **never** depend on:

- pointer values,
- allocation order,
- hash map iteration,
- thread scheduling.

Memory addresses are inherently unstable across runs and platforms.

Identity is derived from structure and explicit intent only.

---

## 5.2.3 Sources of Identity

There are two sources of identity:

1. **Explicit Identity**  
   Provided by the authoring layer via `id: Option<NodeId>`.

2. **Derived Identity**  
   Computed deterministically during lowering when no explicit ID is provided.

Explicit identity always takes precedence.

---

## 5.2.4 Explicit Identity Semantics

Explicit IDs are:

- stable across rebuilds,
- stable across framework versions (subject to author control),
- directly usable as test selectors.

Rules:
- explicit IDs must be unique within their scope,
- conflicts are detected during lowering,
- explicit IDs are never rewritten.

Explicit identity is opt-in and minimal.

---

## 5.2.5 Derived Identity Model

Derived identity is computed from:

- parent identity,
- structural position (ordered index),
- Core IR operation type,
- optional semantic role.

This forms a stable identity path.

Example (conceptual):

```text
root
 └─ group[0]
     └─ flex[2]
         └─ text[0]
```

The same structure always produces the same identities.

---

## 5.2.6 Structural Stability Requirement

Derived identity stability depends on **structural stability**.

Identity is preserved if:
- the node remains at the same structural position,
- sibling ordering is unchanged,
- the surrounding Core ops are equivalent.

If structure changes, identity changes intentionally.

This makes diffs meaningful.

---

## 5.2.7 Identity Boundaries

Certain Core ops introduce identity boundaries:

- `Scope` always introduces a new identity root,
- structural grouping may or may not, depending on context,
- fragments never introduce identity.

Identity boundaries prevent unrelated changes from cascading.

---

## 5.2.8 Identity and Canonicalization

Identity assignment occurs **before** canonicalization.

Canonicalization must:
- preserve identity where semantics are unchanged,
- avoid reassigning IDs,
- only remove identities when nodes are eliminated entirely.

Identity is part of the semantic contract.

---

## 5.2.9 Stable Node ID Representation

Node IDs are represented as:

- opaque, strongly typed identifiers,
- serializable and comparable,
- printable for debugging.

They are notzen:
- not human-authored strings by default,
- not tied to Rust types or symbols,
- not platform-specific.

Human-readable aliases may be layered on top.

---

## 5.2.10 Relationship to Selectors and Tests

Tests rely on identity stability to support APIs such as:

```rust
find(node_id).rect()
find(role("button")).action("increment")
```

Because identity is deterministic:
- selectors are stable,
- tests are non-flaky,
- replay is reliable.

---

## 5.2.11 Error Handling and Diagnostics

Identity-related errors include:

- duplicate explicit IDs,
- invalid ID scopes,
- identity collisions after canonicalization.

Errors are:
- detected during lowering,
- reported with provenance,
- deterministic and reproducible.

---

## 5.2.12 Summary

Deterministic identity and stable Node IDs:

- are required for correctness and testability,
- are derived structurally, not operationally,
- survive rebuilds when structure is stable,
- intentionally change when semantics change.

This identity model enables the framework’s guarantees end-to-end.

---
