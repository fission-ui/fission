# 10.6 Lazy Lists and Future Extensions

This section outlines how **lazy lists** and other advanced scrolling constructs fit into the scrolling model without violating determinism, testability, or Core simplicity.
Lazy behavior is an optimization strategy layered on top of explicit scroll semantics—not a different model.

---

## 10.6.1 Motivation for Lazy Lists

Lazy lists exist to:

- handle very large or unbounded content,
- reduce layout and paint work,
- maintain smooth scrolling performance.

However, traditional lazy implementations often introduce:
- hidden state,
- non-reproducible behavior,
- input- and timing-dependent layouts.

These pitfalls are explicitly avoided.

---

## 10.6.2 Lazy Lists as a Derived Optimization

In this framework, lazy lists are:

- semantically identical to fully realized lists,
- observationally equivalent in snapshots,
- implemented as an optimization layer.

From the Core’s perspective, a lazy list is still a scroll container with content.

---

## 10.6.3 Explicit Windowing Model

Lazy lists use an explicit **windowing model**.

The window is defined by:
- current scroll offset,
- viewport size,
- configured prefetch margins.

Window bounds are explicit data and inspectable.

---

## 10.6.4 Deterministic Item Materialization

Item materialization rules:

- are driven solely by window bounds,
- are independent of frame timing,
- do not depend on render performance,
- are deterministic for identical inputs.

Materialization order is explicit and stable.

---

## 10.6.5 Placeholder Geometry

Non-materialized items still participate in layout via placeholders.

Rules:
- placeholders provide deterministic size estimates,
- size estimation is explicit and versioned,
- replacing placeholders does not shift unrelated items.

This preserves scroll extents and offsets.

---

## 10.6.6 Interaction With Layout and Painting

Lazy behavior affects:

- which nodes are present in the Core IR,
- which nodes produce paint ops.

It does not affect:
- scroll extents,
- scroll offsets,
- coordinate transforms.

Thus, scroll semantics remain intact.

---

## 10.6.7 Testing Lazy Lists

Lazy lists are testable.

Tests may assert:
- which items are materialized,
- placeholder geometry correctness,
- stability of scroll offsets under materialization,
- visibility transitions as scrolling occurs.

Example:

```rust
assert!(find("item_5").is_materialized());
assert!(!find("item_500").is_materialized());
```

---

## 10.6.8 Accessibility Considerations

Accessibility systems require:

- knowledge of full content size,
- ability to navigate to off-screen items,
- predictable reading order.

Lazy lists expose full semantic ranges even when items are not materialized.

---

## 10.6.9 Future Extensions

The scroll model supports future extensions such as:

- bidirectional infinite lists,
- sectioned or grouped scrolling,
- virtualized grids,
- predictive prefetching,
- LLM-guided viewport reasoning.

All extensions must:
- preserve explicit state,
- remain deterministic,
- integrate with snapshots and tests.

---

## 10.6.10 Non-Goals

Lazy lists must not:

- introduce hidden timers or background threads,
- depend on rendering speed,
- change semantics based on device performance.

Performance optimizations must never alter observable behavior.

---

## 10.6.11 Summary

Lazy lists and future scrolling extensions:

- are layered optimizations, not semantic changes,
- preserve determinism and testability,
- integrate cleanly with scroll state and snapshots,
- allow the framework to scale without complexity leaks.

Scrolling remains explicit—even when content is infinite.

---
