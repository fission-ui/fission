# 10.5 Testing Scroll Behavior

This section defines the testing model for scrolling.
Because scroll state, layout, and painting are all explicit and deterministic, scroll behavior can be tested precisely without gestures, timers, or platform dependencies.

Scroll tests assert *state, geometry, and visibility*, not animations or pixels by default.

---

## 10.5.1 Goals of Scroll Testing

Scroll testing exists to:

- verify correct scroll extents and offsets,
- ensure deterministic responses to input actions,
- validate visibility and clipping behavior,
- test accessibility-driven scrolling,
- detect regressions in layout–scroll interactions.

Scroll tests must be fast, stable, and headless.

---

## 10.5.2 Test Harness Model

Scroll tests run against the standard test harness:

- application state is constructed explicitly,
- layout and painting run headlessly,
- snapshots are captured after each step.

There is no distinction between “unit” and “integration” scroll tests.

---

## 10.5.3 Asserting Scroll State

Tests may assert scroll state directly.

Examples:

```rust
assert_eq!(find("list").scroll_offset(), 0);
assert_eq!(find("list").scroll_extent().max(), 1200);
```

These assertions do not require rendering.

---

## 10.5.4 Driving Scroll via Actions

Scroll behavior is driven by actions.

Examples:

```rust
dispatch(ScrollBy { delta: 200 });
dispatch(Tick { dt: 16 });
```

Actions are deterministic and recorded in traces.

---

## 10.5.5 Visibility and Clipping Assertions

Tests may assert visibility derived from snapshots.

Examples:

```rust
assert!(find("item_5").is_visible());
assert!(!find("item_20").is_visible());
```

Visibility is derived from clip bounds and geometry, not heuristics.

---

## 10.5.6 Geometry Assertions Under Scroll

Geometry queries automatically account for scroll state.

Examples:

```rust
let rect = find("item_3").global_rect();
assert_eq!(rect.y(), 40);
```

Tests can assert both content-space and viewport-space coordinates.

---

## 10.5.7 Paint and Damage Assertions (Optional)

For advanced tests, painting artifacts may be inspected.

Examples:
- asserting that clipped items produce no paint ops,
- checking paint spans for visible ranges.

These tests operate on display lists and PaintMap, not pixels.

---

## 10.5.8 Accessibility Scroll Testing

Accessibility-triggered scrolling is tested identically.

Examples:

```rust
dispatch(AccessibilityScrollForward);
assert_eq!(find("list").scroll_offset(), 400);
```

There is no separate accessibility testing mode.

---

## 10.5.9 Physics and Time Control

Scroll physics tests control time explicitly.

Examples:

```rust
dispatch(Fling { velocity: 800 });
dispatch(Tick { dt: 16 });
dispatch(Tick { dt: 16 });
```

Tests assert exact numeric outcomes.

---

## 10.5.10 Snapshot-Based Regression Tests

Scroll regressions are detected via snapshot diffs.

Tests may compare:
- scroll offsets,
- visible node sets,
- layout geometry under scroll,
- paint spans.

Snapshots make regressions obvious and explainable.

---

## 10.5.11 Common Failure Modes Detected

Scroll tests can catch:

- incorrect extents,
- nondeterministic physics,
- layout reflow on scroll,
- incorrect clipping,
- accessibility mismatches.

These failures are often missed in gesture-driven tests.

---

## 10.5.12 Summary

Testing scroll behavior is straightforward because:

- scroll state is explicit,
- actions are deterministic,
- geometry and visibility are inspectable,
- snapshots capture all relevant data.

Scroll tests validate behavior, not implementation details.

---
