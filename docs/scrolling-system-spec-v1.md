# Scrolling System Spec (v1)

This document defines the **v1 scrolling model** for the framework.
Scrolling is treated as a **first-class, deterministic runtime capability** that integrates with:
- layout and constraints,
- hit testing and input routing,
- painting and clipping,
- semantics and accessibility,
- headless testing and CI.

The design goal is **determinism**: given the same initial state, the same inputs, and the same runtime clock, scroll output must be identical.

---

## 1. Goals and Non-Goals

### Goals
- Provide a scalable scrolling primitive usable for:
  - single-child scroll views,
  - nested scrolling (future),
  - lists (future extension).
- Deterministic behavior across platforms and CI.
- Full test control: set offset, inject wheel/drag, advance time, assert offset/visibility.
- Clear separation of responsibilities:
  - layout defines viewport and content extents,
  - runtime owns scroll offset and physics,
  - paint compiles clipping and translation.

### Non-Goals (v1)
- Virtualized / lazy lists (future section).
- Complex nested scroll coordination (future).
- Platform-native scroll views (the core owns behavior for determinism).

---

## 2. Core Concepts

### 2.1 Viewport vs Content
A scroll container defines:
- a **viewport rect** (what is visible),
- a **content extent** (the scrollable size of child content),
- a **scroll offset** (runtime state) clamped to valid range.

### 2.2 Scroll Offset as Runtime State
Scroll offset is **not** stored in user `AppState` by default.
It is part of `RuntimeState`, keyed by stable `WidgetNodeId`.

This mirrors other transient UI state (hover/press/focus).

### 2.3 Deterministic Scroll Physics
Scroll physics must be deterministic:
- friction/inertia must use the framework-owned clock,
- fixed timestep or deterministic integration,
- no reliance on OS scroll momentum.

---

## 3. Authoring API: `Scroll` Widget

A basic single-child scroll widget:

```rust
#[derive(Default, Clone, Serialize, Deserialize)]
pub struct Scroll {
    pub id: Option<WidgetNodeId>,
    pub axis: Axis,                 // Vertical/Horizontal/Both (v1: one axis recommended)
    pub child: Option<Box<Node>>,
    pub physics: ScrollPhysics,      // deterministic parameters
    pub clip: ClipBehavior,          // hard clip by default
}
```

### 3.1 Axis
v1 should strongly prefer a single axis:
- `Axis::Vertical` or `Axis::Horizontal`.

Bidirectional scroll can be future extension.

### 3.2 ScrollPhysics
Represent physics as pure data:

```rust
pub struct ScrollPhysics {
    pub overscroll: Overscroll,  // clamp vs bounce (deterministic)
    pub friction: f32,
    pub wheel_step: f32,
}
```

No platform-dependent parameters.

---

## 4. IR Design: `LayoutOp::Scroll` (Preferred)

For determinism and tooling, scrolling should be explicit in IR rather than implicit via arbitrary clip+translate patterns.

### 4.1 Layout Op
Introduce:

```rust
LayoutOp::Scroll {
    id: WidgetNodeId,
    axis: Axis,
    child: NodeId,
}
```

Layout responsibilities:
- lay out the child with unbounded constraints along the scroll axis (or large max),
- compute:
  - viewport size,
  - child content extent,
  - scrollable range.

Outputs for instrumentation:
- `viewport_rect`
- `content_extent`
- `scroll_range`
- stable mapping from node id → these values

### 4.2 Paint Compilation
During paint compilation, `LayoutOp::Scroll` becomes a **paint stack**:

1) `PushClipRect(viewport_rect)`
2) `PushTransform(translation = -scroll_offset along axis)`
3) paint child subtree
4) `PopTransform`
5) `PopClipRect`

This makes the relationship explicit and guarantees correct clipping.

---

## 5. Alternative IR: Clip + Translation (Allowed but Not Preferred)

It is possible to represent scroll purely as:
- clip rect
- translation transform

However, encoding scroll as a convention (instead of a dedicated op) makes:
- instrumentation harder,
- semantics integration harder,
- debug and test APIs less direct.

Therefore:
- v1 should expose a **first-class scroll op** even if the renderer ultimately uses clip+translate.

---

## 6. RuntimeState Integration

### 6.1 Scroll State Map
Runtime stores scroll state keyed by `WidgetNodeId`:

```rust
pub struct RuntimeScrollState {
    pub offset: f32,
    pub velocity: f32, // if physics enabled
}

pub struct RuntimeState {
    pub scroll: BTreeMap<WidgetNodeId, RuntimeScrollState>,
    // ...
}
```

Determinism requirements:
- stable map type (e.g., BTreeMap) or stable hashing,
- explicit update ordering.

### 6.2 Update Rules
The runtime updates scroll offsets from:
- pointer drag gestures,
- wheel events,
- keyboard/page navigation,
- physics integration.

All updates are:
- clamped to scroll range,
- applied deterministically.

---

## 7. Input Routing and Hit Testing

### 7.1 Pointer and Wheel
- Pointer drags inside the scroll viewport may initiate scrolling.
- Wheel events target the nearest scrollable ancestor.

### 7.2 Coordinate Spaces
Hit testing must account for scroll transforms:
- event coordinates are transformed into child space using current offset.

### 7.3 Deterministic Gesture Resolution
Gesture recognition must be deterministic:
- same sequence of pointer events yields same capture/scroll decision.

---

## 8. Semantics and Accessibility

A scroll container must expose semantics:
- role: `ScrollView` (or equivalent)
- scroll actions:
  - scroll up/down/left/right by step
  - page by viewport
- current scroll position (if useful)

Platform accessibility bridges map these to native APIs.

---

## 9. Testing APIs

The test harness must support:

### 9.1 Direct Control
- `find("list").scroll_offset()` → current offset
- `find("list").set_scroll_offset(120.0)` → immediate
- `find("list").scroll_by(50.0)` → deterministic delta

### 9.2 Event Simulation
- `wheel(delta)`
- `drag(from, to, steps)`
- `flick(velocity)`

### 9.3 Visibility Assertions
- `find("item_42").is_visible()`
- `find("item_42").rect_in_viewport()`

### 9.4 Snapshot Coverage
Scroll state is included in snapshots:
- per scroll id: offset, velocity (if applicable)

This enables replay and stable CI.

---

## 10. Future Extensions

### 10.1 Lazy Lists
Introduce a `LazyList` widget that:
- only realizes visible children,
- participates in the same scroll model,
- produces stable item identity.

### 10.2 Nested Scroll Coordination
Add deterministic arbitration rules for:
- inner vs outer scroll views
- trackpad gestures
- overscroll chaining

---

## 11. Summary

- Scrolling is first-class and deterministic.
- Scroll offset lives in `RuntimeState`, keyed by stable ids.
- Prefer a dedicated `LayoutOp::Scroll`.
- Paint compiles scroll to `ClipRect + Translate(offset)`.
- Hit testing and semantics integrate directly with scroll op.
- Tests can control and assert scroll state precisely.
