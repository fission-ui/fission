# 10. Scrolling

This section defines the scrolling model as a first-class, deterministic system spanning layout, input, semantics, painting, and testing.
Scrolling is treated as explicit state and geometry transformation, not an implicit side effect.

Scrolling answers one question only: *how content is translated and clipped relative to a viewport*.

---

## 10.1 Design Goals

The scrolling system is designed to:

- be fully deterministic and testable,
- expose scroll state explicitly,
- compose with layout and painting cleanly,
- support accessibility and keyboard navigation,
- work identically in headless and on-screen modes.

Scrolling must never trigger hidden re-layouts.

---

## 10.2 Scroll State as Explicit Data

Scroll position is explicit state.

Properties:
- owned by the runtime state model,
- represented in logical units,
- versioned and observable,
- serializable and replayable.

There is no implicit scroll position derived from input.

---

## 10.3 Scroll Containers and Viewports

A scroll container defines:

- a viewport rect,
- a content extent,
- one or more scroll axes,
- clipping semantics.

The viewport is a hard boundary for visibility and hit testing.

---

## 10.4 Content Layout Independence

Rules:
- content is laid out at full extent,
- viewport size does not constrain content layout along the scroll axis,
- scrolling applies a translation to geometry.

This guarantees stable content geometry across scroll positions.

---

## 10.5 Scroll Extents and Bounds

Scroll extents are computed deterministically from:

- content size,
- viewport size,
- axis configuration.

Rules:
- extents are explicit and inspectable,
- invalid offsets are errors,
- clamping behavior is explicit and configurable.

---

## 10.6 Input to Scroll Actions

Scrolling is driven by actions.

Examples:
- `ScrollBy { delta }`
- `ScrollTo { position }`
- `PageUp`, `PageDown`

Input devices never mutate scroll state directly.

---

## 10.7 Accessibility and Scroll Semantics

Scrolling exposes semantics:

- scrollable role,
- current offset and range,
- increment/decrement actions,
- page navigation actions.

Accessibility systems invoke the same actions as tests and input.

---

## 10.8 Focus and Keyboard Navigation

Scroll containers participate in focus management.

Rules:
- focused elements may request visibility,
- scroll actions may be triggered by focus movement,
- focus traversal is deterministic and observable.

Auto-scrolling is explicit and traceable.

---

## 10.9 Painting and Clipping

Scrolling affects painting via:

- geometry translation,
- clip bounds at the viewport.

Rules:
- paint order is unchanged,
- clipping is applied after translation,
- off-viewport content remains in the display list.

---

## 10.10 Hit Testing

Hit testing under scrolling:

- transforms input coordinates into content space,
- respects clip bounds,
- maps hits back to NodeIds via PaintMap.

Hit testing is deterministic and testable.

---

## 10.11 Testing Scrolling

Scrolling is fully testable headlessly.

Tests may assert:
- scroll extents,
- offsets after actions,
- visible content ranges,
- focus-driven scrolling behavior.

Example:

```rust
dispatch(ScrollBy { delta: 100 });
assert_eq!(find("list").scroll_offset(), 100);
```

---

## 10.12 Determinism Guarantees

Scrolling is deterministic because:

- scroll state is explicit,
- geometry transforms are pure,
- no reflow occurs on scroll,
- input is modeled as actions.

Identical traces yield identical outcomes.

---

## 10.13 Instrumentation and Replay

Scroll interactions appear in action traces.

This enables:
- replaying scroll behavior,
- debugging scroll-related issues,
- CI validation of interaction flows.

Scrolling is not special-cased.

---

## 10.14 Summary

The scrolling system:

- treats scroll as explicit state,
- composes cleanly with layout and painting,
- supports accessibility and testing,
- guarantees deterministic behavior across platforms.

Scrolling is geometry plus state—nothing more, nothing less.

---
