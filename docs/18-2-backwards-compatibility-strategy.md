# 18.2 Backwards Compatibility Strategy

This section defines the framework’s **backwards compatibility guarantees** and how evolution is managed
without breaking existing applications, tests, or tools.
Compatibility is explicit, versioned, and enforced by policy—not left to convention.

Breaking behavior is a conscious act, never an accident.

---

## 18.2.1 Scope of Compatibility

Backwards compatibility applies to:

- Core IR semantics and operation meanings,
- snapshot formats (with versioning),
- action identity and dispatch behavior,
- layout and rounding rules,
- accessibility semantics,
- test APIs and selectors.

Compatibility guarantees do **not** extend to performance characteristics.

---

## 18.2.2 Core IR Versioning

The Core IR is versioned explicitly.

Rules:
- each Core IR version has a fixed, documented semantics,
- ops are never reinterpreted,
- deprecated ops remain valid indefinitely or until a major version bump,
- new ops are added conservatively.

Core IR versions are immutable once released.

---

## 18.2.3 Semantic Stability Guarantees

Semantic meaning is sacred.

Guarantees include:
- identical Core IR produces identical snapshots,
- identical snapshots produce identical display lists,
- identical display lists render identically (per renderer contract),
- identical actions produce identical state transitions.

Semantic changes require a new major version.

---

## 18.2.4 Snapshot and Diff Format Evolution

Snapshots and diffs are versioned data formats.

Rules:
- snapshot headers declare format versions,
- readers must support older versions where feasible,
- migrations are deterministic and explicit,
- tooling can inspect mixed-version artifacts.

Old snapshots remain readable even as formats evolve.

---

## 18.2.5 Action Compatibility

Actions are part of the compatibility surface.

Rules:
- action tags are stable identifiers,
- payload schemas are versioned,
- removed actions are deprecated, not reused,
- renamed actions use aliases.

Action identity stability enables long-lived tests and replays.

---

## 18.2.6 Layout and Geometry Stability

Layout rules are highly sensitive.

Guarantees:
- rounding rules never change within a major version,
- constraint resolution semantics are fixed,
- baseline calculations are stable.

Layout changes require explicit version opt-in.

---

## 18.2.7 Accessibility Compatibility

Accessibility compatibility includes:

- stable role mappings,
- stable traversal rules,
- stable action semantics.

Enhancements may add metadata but must not remove meaning.

---

## 18.2.8 Authoring Layer Compatibility

The authoring layer evolves faster than the Core.

Rules:
- authoring APIs are backwards compatible within a major version,
- deprecations are warned and documented,
- automatic migrations are preferred.

Authoring changes never alter lowered Core semantics silently.

---

## 18.2.9 Renderer Compatibility

Renderer behavior is validated against reference outputs.

Rules:
- renderers must respect display list semantics,
- renderer upgrades must not change output for existing display lists,
- discrepancies are treated as bugs.

Renderer changes do not affect Core compatibility guarantees.

---

## 18.2.10 Version Pinning and Opt-In Upgrades

Applications can pin versions explicitly.

Capabilities:
- pin Core IR version,
- pin snapshot format version,
- opt into newer versions incrementally.

Upgrades are explicit, not forced.

---

## 18.2.11 Deprecation Policy

Deprecation follows a strict process:

1. announce deprecation with rationale,
2. provide migration guidance,
3. support deprecated behavior for a defined window,
4. remove only in a major version bump.

Silent removal is forbidden.

---

## 18.2.12 Compatibility Testing

Compatibility is enforced by tests.

Strategies:
- golden snapshot tests across versions,
- action replay tests on old recordings,
- cross-version diff validation,
- renderer parity tests.

Compatibility failures block releases.

---

## 18.2.13 Tooling and Ecosystem Stability

Tools depend on compatibility guarantees.

Rules:
- inspectors and debuggers target stable snapshot schemas,
- CI tooling supports multiple versions concurrently,
- LLM tooling relies on stable semantics.

Tool breakage is treated as a compatibility regression.

---

## 18.2.14 Long-Term Guarantees

The framework commits to:

- long-lived Core IR versions,
- conservative semantic evolution,
- explicit and predictable upgrades.

Stability is a feature, not a cost.

---

## 18.2.15 Summary

Backwards compatibility is achieved because:

- semantics are versioned and immutable,
- evolution is explicit and opt-in,
- tooling enforces guarantees,
- discipline is architectural, not cultural.

Users can upgrade with confidence—and choose when not to.
