# 14. Testing and Instrumentation

This section defines the **testing and instrumentation philosophy and architecture** of the framework.
Testing is not an add-on; it is a first-class capability designed into every layer, from Core IR through rendering.

Instrumentation exists to make *all behavior observable, queryable, and reproducible*.

---

## 14.1 Core Principles

Testing and instrumentation are governed by four principles:

1. **Determinism first** – tests must never flake.
2. **Observability over heuristics** – expose data, not guesses.
3. **Uniform APIs** – the same APIs are used by humans, CI, and LLMs.
4. **Zero-cost when disabled** – instrumentation must not penalize production builds.

---

## 14.2 Unified Test Model

There is a single test model for the entire framework.

Tests:
- construct application state,
- dispatch actions,
- advance time explicitly,
- capture snapshots,
- assert over structured data.

There are no separate “unit”, “widget”, or “integration” test modes.

---

## 14.3 Instrumentation as a Layer, Not a Fork

Instrumentation is an optional layer that:

- subscribes to Core events,
- observes snapshots,
- records traces,
- exposes query APIs.

When disabled:
- no extra allocations occur,
- no virtual dispatch is added,
- no behavior changes.

---

## 14.4 Snapshot-Centric Testing

Snapshots are the primary testing artifact.

Snapshots expose:
- structure (trees and ordering),
- geometry (rects, baselines, bounds),
- semantics and accessibility data,
- animation and media state,
- paint and render descriptions.

Assertions operate on snapshots, not pixels by default.

---

## 14.5 Structural Queries

Tests can query structure deterministically.

Examples:

```rust
find("button").parent();
find("list").children();
find("item_3").index_in_parent();
```

Structural queries never depend on rendering.

---

## 14.6 Geometry Queries

Geometry is fully observable.

Examples:

```rust
find("title").rect();
find("text").baseline();
find("image").paint_bounds();
```

All geometry values follow explicit rounding rules.

---

## 14.7 Visibility and Hit Testing Queries

Visibility is derived from snapshots.

Examples:

```rust
assert!(find("item").is_visible());
assert!(find("button").hit_test(10, 10));
```

Hit testing is deterministic and snapshot-driven.

---

## 14.8 Action Tracing and Replay

All actions are traceable.

Traces include:
- dispatched actions,
- timestamps or ticks,
- reducer outcomes,
- resulting snapshots.

Traces can be replayed exactly to reproduce bugs.

---

## 14.9 Time Control in Tests

Tests own time completely.

Examples:

```rust
dispatch(Tick { dt: 16 });
dispatch(Tick { dt: 16 });
```

There is no concept of “waiting” in tests.

---

## 14.10 Golden Testing

Golden tests are supported but optional.

Rules:
- golden tests are deterministic,
- pinned fonts, images, and decoders are used,
- pixel diffs are structural and explainable.

Golden tests complement snapshot tests.

---

## 14.11 Fault Injection

Instrumentation supports fault injection.

Examples:
- media load failures,
- layout overflow,
- animation cancellation,
- backend errors.

Faults are deterministic and replayable.

---

## 14.12 LLM-Oriented Testing APIs

Testing APIs are designed to be LLM-friendly.

Properties:
- small surface area,
- declarative queries,
- no callbacks or async control flow.

LLMs can reason about UI behavior directly from snapshots.

---

## 14.13 Performance and Scale

Instrumentation is designed to scale.

Rules:
- snapshots are compact and diffable,
- tracing is configurable and sampled,
- CI performance is predictable.

Large applications remain testable.

---

## 14.14 Security and Isolation

Instrumentation does not bypass security.

Rules:
- no access to private platform APIs,
- no hidden IO,
- explicit permissions for inspection hooks.

Tests cannot observe what the app itself cannot.

---

## 14.15 Summary

Testing and instrumentation are foundational because:

- behavior is deterministic,
- state is observable,
- time is controlled,
- failures are reproducible.

This framework treats correctness as a feature—not an afterthought.

---
