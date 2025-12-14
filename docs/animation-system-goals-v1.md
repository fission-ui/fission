# Animation System Goals and Architecture (v1)

This document defines the goals, scope, and architectural role of animations in the framework.  
It clarifies what the animation system **is**, what it **is not**, and how external animation formats
(e.g. Lottie) fit into the model without compromising determinism or testability.

---

## 1. Primary Goal: Deterministic, State-Driven Animations

The core goal of the animation system is:

> **Animation is deterministic state evolution over an explicit, framework-owned clock.**

This goal follows directly from the framework’s core principles:
- UI = f(State)
- Determinism is non-negotiable
- Headless execution and CI testing must be reliable
- All runtime behavior must be observable and replayable

### 1.1 What “deterministic animation” means

- The framework **owns time**
- Animation evaluation is pure and repeatable
- No animation logic lives in the OS, renderer, or platform shell
- Advancing time by the same amount always produces the same visual result

Conceptually:

```text
Animation = {
  start_value,
  end_value,
  easing,
  duration,
  start_time
}

value_at_time(t) = evaluate(animation, t)
```

No callbacks, no hidden threads, no imperative timelines.

---

## 2. What the Core Animation System Supports (v1)

The built-in animation system is intended to support **UI and interaction animation**, including:

### 2.1 Property animations
- Opacity
- Position / translation
- Size
- Scale / transform
- Color
- Elevation / shadow parameters

### 2.2 Layout-aware animations
- Animated constraint changes
- Animated flex/grid reflow
- Animated intrinsic size changes

### 2.3 Interaction-driven animations
- Hover → pressed → released
- Focus transitions
- Enabled / disabled transitions

### 2.4 Scroll physics
- Scroll offsets as animated state
- Velocity, friction, snapping
- Driven by the same owned clock

### 2.5 Test control
Animations must be fully controllable in tests:

```rust
harness.advance_time(200.ms());
assert_eq!(find("button").opacity(), 0.5);
```

This is a **hard requirement**, not a convenience feature.

---

## 3. Explicit Non-Goal: Replacing Lottie or Artist Pipelines

The framework does **not** aim to replace Lottie, After Effects, or similar animation-authoring tools.

Reasons:
- Those tools are timeline-based and artist-oriented
- They often produce platform-dependent results
- They are difficult to snapshot, diff, and test deterministically
- They introduce heavy runtime and asset dependencies

Therefore:

> **The framework is not a Lottie competitor.**

---

## 4. Correct Model: Lottie as an Embed, Not a Primitive

If Lottie (or similar formats) are supported, they must be treated as **media embeds**, not core primitives.

### 4.1 Architectural position

- Lottie lives behind `Node::Custom` or an `EmbedNode`
- It is driven by the **same framework-owned clock**
- It exposes explicit, testable state

Conceptual model:

```text
LottieEmbed {
  asset_id,
  duration,
  playback_state,
  time_source: RuntimeClock,
}
```

### 4.2 What Lottie embeds must provide

- Intrinsic size
- Deterministic frame evaluation
- Progress (0.0 → 1.0) derived from framework time
- Optional semantics (if interactive)

### 4.3 What Lottie embeds must NOT do

- Start their own timers
- Advance frames internally
- Depend on platform animation APIs
- Bypass the framework’s clock or test harness

---

## 5. How Animations Integrate with the Core Architecture

### 5.1 Runtime ownership

- Animations are **runtime state**, not authoring-time constructs
- Widgets declare *what* animates, not *how time advances*
- The runtime evaluates animation values each frame

### 5.2 Core IR interaction

- Core IR references animated properties symbolically
- Evaluation occurs before paint compilation
- Display lists always see resolved values

### 5.3 Custom widgets

- Custom widgets may:
  - register animations declaratively
  - emit animated Core IR via `Node::Custom`
- All custom animation logic must obey determinism constraints

---

## 6. Determinism Requirements for Animations

All animations must obey the following:

### Forbidden
- Wall-clock time
- OS animation frameworks
- Renderer-driven animation
- Randomness without explicit seeding

### Required
- Explicit start time
- Explicit duration
- Explicit easing
- Evaluation based only on framework-owned clock

---

## 7. Summary

- **Primary goal**: deterministic, state-driven, testable UI animation
- **Core animations**: first-class, owned by the runtime
- **Lottie**: optional embed, not a primitive
- **Key invariant**: the framework always owns time

This model ensures:
- reliable CI
- replayable interaction traces
- stable snapshots
- consistent behavior across platforms
