# 8.3 Flex Layout Semantics

This section defines the semantics of flex layout within the layout system.
Flex layout is used to distribute available space among children in a single axis deterministically, with explicit rules for sizing, ordering, and alignment.

Flex layout is declarative and free of heuristics.

---

## 8.3.1 Purpose of Flex Layout

Flex layout exists to:

- arrange children along a main axis,
- distribute remaining space proportionally,
- support intrinsic, fixed, and flexible sizing,
- provide predictable cross-axis alignment.

Flex is not a general constraint solver; it is a specific, well-defined model.

---

## 8.3.2 Axes and Orientation

Each flex container defines:

- a **main axis** (horizontal or vertical),
- a **cross axis** (perpendicular to the main axis).

All sizing and distribution rules are defined relative to these axes.

---

## 8.3.3 Child Ordering

Child order is semantic.

Rules:
- children are laid out strictly in declared order,
- reordering children changes layout deterministically,
- keyed nodes preserve identity but not order semantics.

Flex never reorders children.

---

## 8.3.4 Size Contributions

Each child contributes to size in one of three ways:

1. **Fixed Size**  
   Explicit size along the main axis.

2. **Intrinsic Size**  
   Measured size bounded by constraints.

3. **Flexible Size**  
   Size determined by flex factor.

Children may mix modes across axes.

---

## 8.3.5 Flex Factors

Flexible children declare a non-negative flex factor.

Rules:
- flex factors are normalized across flexible children,
- remaining space is divided proportionally,
- zero flex means no participation in distribution.

Normalization is deterministic and order-independent.

---

## 8.3.6 Distribution Algorithm

The distribution algorithm proceeds as follows:

1. Measure fixed and intrinsic children.
2. Subtract their sizes from available space.
3. Normalize flex factors.
4. Allocate remaining space proportionally.
5. Apply min/max constraints.
6. Record overflow if space is insufficient.

No rounding occurs during distribution.

---

## 8.3.7 Cross-Axis Sizing and Alignment

Cross-axis size is determined by:

- explicit constraints,
- maximum child size (if stretch),
- intrinsic size (if wrap).

Alignment options include:
- start,
- center,
- end,
- stretch,
- baseline (where applicable).

Alignment semantics are explicit and deterministic.

---

## 8.3.8 Baseline Alignment

Baseline alignment applies only when:

- the main axis is horizontal,
- children provide baselines.

Rules:
- baseline offsets are computed explicitly,
- missing baselines are treated deterministically,
- baseline alignment does not affect main-axis sizing.

---

## 8.3.9 Overflow Handling

If total child size exceeds available space:

- overflow is recorded explicitly,
- children are not reflowed,
- clipping is applied later by layout rules.

Overflow behavior is observable and testable.

---

## 8.3.10 Interaction With Scrolling

Flex layout may be nested within scroll containers.

Rules:
- flex computes full content size,
- scroll applies translation and clipping,
- flex semantics are unaffected by scrolling.

---

## 8.3.11 Determinism Guarantees

Flex layout is deterministic because:

- child order is fixed,
- arithmetic is explicit,
- no heuristics or backtracking exist.

Identical inputs yield identical layouts.

---

## 8.3.12 Testing Flex Layout

Flex layout is testable headlessly.

Tests may assert:
- child sizes and positions,
- proportional distribution,
- alignment behavior,
- overflow conditions.

Example:

```rust
assert_eq!(find("item_a").rect().width(), 100);
```

---

## 8.3.13 Summary

Flex layout semantics:

- provide predictable space distribution,
- support mixed sizing strategies,
- preserve order and identity,
- remain deterministic and testable.

Flex is a core layout primitive built for correctness, not convenience.

---
