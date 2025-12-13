# 4.5 Versioning and Forward Compatibility

This section defines how the Core IR is versioned and how forward and backward compatibility are preserved.
Because the Core IR is the semantic contract of the framework, changes to it must be deliberate, explicit, and survivable over time.

Versioning is not an implementation detail; it is part of the Core design.

---

## 4.5.1 The Core IR as a Compatibility Boundary

The Core IR defines the observable behavior of the UI.

If two framework versions:
- accept the same Core IR,
- interpret it under the same version rules,

they must produce identical semantics.

This makes the Core IR the primary compatibility boundary, not the authoring APIs.

---

## 4.5.2 Explicit Core IR Versioning

Every Core IR instance is associated with an explicit version.

The version:
- is included in serialized representations,
- is available in snapshots and traces,
- is checked during deserialization.

There is no implicit or “latest” interpretation.

---

## 4.5.3 Version Granularity

Versioning applies at the Core IR level, not per widget or per feature.

A Core IR version defines:
- the set of valid operations,
- the meaning of each operation,
- canonicalization rules,
- default behaviors.

This avoids combinatorial complexity.

---

## 4.5.4 Backward Compatibility Guarantees

For released Core IR versions, the framework guarantees:

- older Core IR can be loaded by newer runtimes,
- semantics are preserved exactly,
- canonicalization produces equivalent results.

Backward compatibility is mandatory unless explicitly broken by a major version change.

---

## 4.5.5 Forward Compatibility Strategy

Forward compatibility is supported via:

- unknown-field tolerance,
- explicit version checks,
- conservative interpretation rules.

A runtime encountering a newer Core IR version may:
- reject it with a clear error, or
- operate in a compatibility mode if supported.

Silent misinterpretation is forbidden.

---

## 4.5.6 Adding New Fields

When extending existing Core IR ops:

- new fields must be optional or have defaults,
- defaults must preserve existing behavior,
- canonicalization rules must be updated accordingly.

Adding required fields is a breaking change.

---

## 4.5.7 Adding New Operations

Adding a new Core IR operation:

- is always a compatibility-sensitive change,
- requires version increment,
- must be justified as a new fundamental capability.

New operations must not invalidate existing canonical forms.

---

## 4.5.8 Deprecation and Removal

Deprecation follows a staged process:

1. Mark operation or field as deprecated.
2. Continue full support across releases.
3. Provide automated migration where possible.
4. Remove only in a major version.

Removal without migration is strongly discouraged.

---

## 4.5.9 Canonicalization and Versioning

Canonicalization rules are versioned.

This ensures:
- older Core IR canonicalizes the same way forever,
- newer rules do not retroactively change meaning.

Canonicalization is part of the semantic contract.

---

## 4.5.10 Testing Compatibility

Compatibility is enforced by tests:

- golden Core IR snapshots,
- cross-version loading tests,
- semantic equivalence tests.

Breaking changes must be detected automatically.

---

## 4.5.11 Relationship to Authoring APIs

Authoring APIs may evolve freely.

As long as:
- they lower into stable Core IR,
- lowering rules are version-aware,

applications and tests remain compatible.

This decoupling is intentional.

---

## 4.5.12 Summary

Versioning and forward compatibility are foundational to the Core IR.

By:
- making versions explicit,
- enforcing strict compatibility rules,
- treating canonicalization as semantic,

the framework ensures that Core IR remains a reliable, long-lived contract.

---
