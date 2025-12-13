# 15.6 Layout Verification Test

This section presents a **layout verification test** for the Counter application.
The goal is to validate the deterministic geometry contract: sizing, positioning, baseline alignment, and paint bounds—without relying on pixel output.

This test verifies that the UI “looks right” through structured geometry assertions.

---

## 15.6.1 Test Intent

The layout verification test must confirm that:

- the button and text are placed as expected,
- spacing between elements is correct,
- baseline information is stable and correct for text,
- paint bounds are sensible (e.g. text paint bounds contain glyphs),
- layout remains stable across state updates.

---

## 15.6.2 Preconditions for Deterministic Layout Tests

Deterministic layout tests require pinned inputs:

- font bundle (and shaping engine) pinned,
- deterministic rounding rules enabled,
- fixed viewport size,
- fixed locale/text direction if relevant.

Without these, geometry tests are invalid.

---

## 15.6.3 Harness Setup

```rust
let mut harness = TestHarness::new(CounterApp::default())
    .with_viewport(Size { w: 320, h: 64 })
    .with_pinned_fonts(FONT_BUNDLE_V1);
```

Then evaluate once:

```rust
harness.pump(); // evaluates pending work without advancing time
```

---

## 15.6.4 Rect Assertions

Verify layout rectangles.

```rust
let button = find("increment_button");
let spacer = find("spacer_0");
let text = find("counter_text");

assert_eq!(button.rect(), Rect { x: 100, y: 16, w: 88, h: 32 });
assert_eq!(spacer.rect().w, 16);
assert_eq!(text.rect().x, button.rect().right() + spacer.rect().w);
```

These are numeric contracts.

---

## 15.6.5 Baseline Assertions

Verify text baseline stability.

```rust
let baseline = text.baseline().expect("text must expose a baseline");
assert_eq!(baseline, 14);
```

Baseline assertions require pinned font metrics.

---

## 15.6.6 Paint Bounds Assertions

Verify that paint bounds are sensible.

```rust
let pb = text.paint_bounds();
assert!(pb.w >= text.rect().w);
assert!(pb.h >= text.rect().h);
```

Paint bounds may exceed layout bounds due to glyph overhang or effects.

---

## 15.6.7 Spacing and Alignment Assertions

Confirm expected spacing between nodes.

```rust
assert_eq!(spacer.rect().w, 16);

let center_y = find("counter_root").rect().center_y();
assert!(button.rect().center_y().abs_diff(center_y) <= 1);
assert!(text.rect().center_y().abs_diff(center_y) <= 1);
```

Alignment tolerances, if any, must be explicit and minimal.

---

## 15.6.8 Layout Stability Across State Changes

Verify that layout does not change when only text content updates.

```rust
let button_before = button.rect();
let text_before = text.rect();

click("increment_button"); // Count: 1

assert_eq!(button.rect(), button_before);
assert_eq!(text.rect(), text_before);
```

If text width changes, it should do so deterministically and be asserted explicitly.

---

## 15.6.9 Snapshot Diff Assertions

Optionally validate that geometry diffs are limited.

```rust
let diff = diff_layout_snapshots(before, after);
assert!(diff.only_affects(["counter_text"], Fields::TextContent));
```

The exact API is illustrative; the key is scoping diffs.

---

## 15.6.10 Failure Diagnostics

On failure, report:

- offending rect/baseline values,
- before/after layout snapshots,
- a structured geometry diff,
- pinned font bundle identifier.

Failures should be immediately explainable.

---

## 15.6.11 Summary

The Counter layout verification test demonstrates that:

- “looks right” can be asserted numerically,
- baselines and paint bounds are first-class,
- layout is deterministic and stable across interactions,
- tests remain fast and renderer-independent.

This is the default approach for layout correctness across the framework.

---
