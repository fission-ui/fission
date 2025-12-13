# 10.1 Scroll as a First-Class Core Primitive

This section explains why scrolling is modeled as a **first-class Core primitive**, rather than an emergent behavior layered on top of layout or input.
Treating scroll explicitly is essential for determinism, testability, accessibility, and replay.

Scrolling is not a gesture; it is stateful geometry.

---

## 10.1.1 Motivation

In many UI frameworks, scrolling is implemented as a side effect of:
- input events (mouse wheel, touch),
- platform widgets,
- implicit view transforms.

This approach introduces:
- hidden state,
- platform divergence,
- nondeterministic behavior,
- poor testability.

To avoid these failures, scrolling is elevated into the Core.

---

## 10.1.2 Scroll as Core Data

In this framework, scrolling is represented as explicit Core data:

- a scroll container node,
- a scroll axis configuration,
- a scroll offset state value,
- explicit scroll extents.

This data participates in:
- lowering,
- layout,
- painting,
- semantics,
- testing.

There is no “special” scrolling code path.

---

## 10.1.3 Separation of Concerns

By making scroll a Core primitive:

- **Layout** computes content size and viewport,
- **Scroll state** determines translation,
- **Painting** applies clipping and transforms,
- **Input** emits scroll actions,
- **Reducers** mutate scroll state.

Each phase has a single responsibility.

---

## 10.1.4 Determinism Benefits

Scroll determinism is guaranteed because:

- scroll offsets are explicit state,
- scroll actions are recorded and replayable,
- no platform scroll physics are implicit,
- geometry transforms are pure.

Identical action traces produce identical scroll behavior.

---

## 10.1.5 Testability Benefits

Scroll as Core data enables tests such as:

```rust
assert_eq!(find("list").scroll_extent().height(), 2400);
dispatch(ScrollTo { offset: 400 });
assert_eq!(find("list").scroll_offset(), 400);
```

No gestures, timers, or platform hooks are required.

---

## 10.1.6 Accessibility Benefits

Accessibility systems require:

- knowing whether content is scrollable,
- knowing current position and range,
- being able to invoke scroll actions.

With scroll as a Core primitive:
- semantics are explicit,
- actions are enumerable,
- behavior is consistent across platforms.

---

## 10.1.7 Composition With Other Core Primitives

Scroll composes cleanly with:

- flex and grid layout,
- clipping and paint bounds,
- hit testing,
- focus traversal,
- animations (scroll-to animations are actions).

No special cases are required.

---

## 10.1.8 Avoiding Platform Scroll Widgets

The framework does not delegate scrolling to:

- native scroll views,
- browser scroll containers,
- OS-level physics engines.

Those systems may exist underneath, but the Core owns the semantics and state.

This preserves cross-platform consistency.

---

## 10.1.9 Implications for Renderer Backends

Renderers are responsible only for:

- applying transforms,
- enforcing clip bounds,
- drawing content.

They do not:
- compute scroll positions,
- apply physics,
- manage scroll state.

Renderer simplicity is a deliberate outcome.

---

## 10.1.10 Long-Term Extensibility

Treating scroll as a primitive enables future features:

- programmable scroll physics,
- scroll snapping,
- synchronized scroll views,
- predictive scrolling,
- LLM-driven scroll reasoning.

All without changing the Core model.

---

## 10.1.11 Summary

Scrolling is a first-class Core primitive because:

- it is stateful and semantic,
- it must be deterministic and replayable,
- it is central to accessibility and testing,
- it composes across the entire pipeline.

By making scroll explicit, the framework avoids an entire class of UI bugs.

---
