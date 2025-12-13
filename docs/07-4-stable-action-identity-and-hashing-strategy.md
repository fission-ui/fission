# 7.4 Stable Action Identity and Hashing Strategy

This section defines how action identity is made stable across builds, platforms, and time, and how hashing is used safely without compromising determinism.
Action identity must be reliable for routing, testing, replay, and long-term compatibility.

Hashes are an implementation detail; stability is the contract.

---

## 7.4.1 Requirements for Action Identity

Action identity must be:

- globally unique within an application,
- stable across recompilations,
- stable across platforms and architectures,
- deterministic across runs,
- serializable and comparable.

Any identity scheme that violates these requirements is unacceptable.

---

## 7.4.2 Identity Is Not a Hash (Conceptually)

While hashes are used internally, **action identity is not defined by a hash value**.

Instead:
- identity is defined by structured metadata,
- hashes are derived from that metadata,
- hashes may change implementation without changing identity.

This distinction prevents accidental semantic breakage.

---

## 7.4.3 Canonical Action Identity Components

The canonical identity of an action consists of:

1. Crate identity (stable crate path)
2. Action type name
3. Optional explicit action version
4. Payload schema fingerprint

These components together define *what the action means*.

---

## 7.4.4 Canonical String Form

Before hashing, action identity is represented in a canonical, textual form.

Example (conceptual):

```text
crate::ui::actions::Increment@v1(payload=())
```

Rules:
- ordering is fixed,
- formatting is versioned,
- no platform-specific data is included.

This canonical form is never exposed publicly but is used internally.

---

## 7.4.5 Hashing Strategy

Hashes are computed from the canonical form.

Requirements:
- deterministic across platforms,
- stable algorithm selection,
- no dependence on pointer values or memory layout.

A cryptographic hash (e.g. BLAKE3 or SHA-256) is preferred for stability over speed.

---

## 7.4.6 Hash Usage

Action hashes are used for:

- fast equality checks,
- lookup in routing tables,
- compact serialization formats.

They are *not* used as the sole source of truth for identity.

---

## 7.4.7 Collision Handling

While cryptographic hashes make collisions vanishingly unlikely, the system is defensive.

Rules:
- identity comparisons fall back to full canonical identity on collision,
- collisions are detectable and diagnosable,
- collisions are treated as fatal errors in debug builds.

Silent collisions are forbidden.

---

## 7.4.8 Versioning and Hash Stability

Changing any canonical identity component:
- changes the canonical form,
- produces a different hash,
- represents a semantic action change.

This is intentional and observable.

Hash stability across framework versions is guaranteed as long as the canonical form rules remain unchanged.

---

## 7.4.9 Relationship to Snapshots and Replay

Because action identity is stable:

- action logs can be replayed across versions,
- snapshots remain comparable,
- tests can assert action sequences reliably.

Hash-based representations remain valid indefinitely.

---

## 7.4.10 Debugging and Diagnostics

For diagnostics, the system can expose:

- canonical identity components,
- debug names,
- hash values (for correlation only).

Debug output never relies on hashes for meaning.

---

## 7.4.11 Why Not Use Enums Alone

Pure enums fail because:

- they do not compose across crates,
- they require central coordination,
- they complicate plugin ecosystems.

The derive-based identity model supports open-world extensibility with closed-world semantics.

---

## 7.4.12 Summary

Stable action identity is achieved by:

- defining identity structurally, not by hash,
- hashing canonical representations deterministically,
- detecting and handling collisions defensively,
- versioning identity rules explicitly.

This strategy ensures actions remain reliable across time, tooling, and platforms.

---
