# Animation API Refinement Spec (v1)

This document refines the v1 animation API to:
- remove string-key hacks (e.g., `pulse_generation`),
- remove author-managed explicit node IDs for typical cases,
- formalize deterministic semantics for animation creation, replacement, and sampling,
- preserve compatibility with the retained / incremental pipeline.

This spec assumes:
- the runtime owns an explicit clock,
- animations are sampled deterministically from that clock,
- UI remains a pure function of explicit inputs (`View`).

---

## 1. Goals and Non-Goals

### Goals
- Make animations ergonomic for large apps without introducing closures or side effects.
- Ensure deterministic replay and headless test control.
- Ensure animations interact cleanly with incremental updates (localized paint dirtiness).
- Eliminate the need for user-defined “unique keys” to retrigger animations.
- Ensure stable targeting without requiring `NodeId::explicit("…")` in normal widget code.

### Non-Goals (v1)
- Multiple concurrent animations blending into the same property of the same node (v2).
- Advanced timeline authoring (Lottie-like authoring remains an embed concern).
- Platform-driven animation clocks.

---

## 2. Identity and Targeting

### 2.1 Stable identity
Animations are retained runtime state and must be keyed by stable identity.

Introduce a stable identifier distinct from arena IDs:

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StableId(u64);
```

Rules:
- `StableId` persists across frames for the “same logical node”.
- `NodeId` (arena id) remains per-frame and must not be used for targeting animations.

### 2.2 Automatic stable ids
The framework assigns stable ids automatically during lowering:
- keyed nodes: derived from explicit keys
- unkeyed nodes: derived from deterministic structural identity rules

Developers supply keys only when identity would otherwise be ambiguous (e.g., list reorder).

### 2.3 Default targeting: “self”
Most animations target the widget/node currently being built.

The framework provides a way to request animations scoped to the current node:

```rust
ctx.anim().request(AnimationRequest { property: ..., ... });
```

Internally, `ctx` attaches the current node’s `StableId` as the target.

Explicit targeting remains possible when needed:

```rust
ctx.anim().request_for(target_id, AnimationRequest { ... });
```

---

## 3. Property Keys

Avoid heap strings for properties.

Introduce interned property atoms:

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct AnimProp(u32);
```

Core property set (v1):
- `opacity`
- `translate_x`, `translate_y`
- `scale`
- `rotation`
- `t` (generic driver value)
- `custom("...")` via stable atom registration (optional)

---

## 4. Runtime Model and Semantics

### 4.1 Single active driver per (target, property)
The runtime maintains:

```rust
active: BTreeMap<(StableId, AnimProp), ActiveAnimation>
```

Semantics:
- There is at most one active animation driving a given property on a given node.
- A new request for the same (target, property) deterministically replaces the existing one.

This does **not** restrict the number of animations in an app:
- different nodes animate independently,
- different properties on the same node animate independently.

### 4.2 Start policies
Animation requests specify how to choose the starting value:

```rust
pub enum StartFrom {
    Explicit(f32),
    Current, // runtime samples last resolved value directly
}
```

This eliminates the need to feed `anim_val` back through the view to compute `from`.

### 4.3 Replacement policy
v1 policy:

- `Replace`: new request replaces the existing animation immediately.

Optionally, also support:
- `IgnoreIfActive` (useful for spammy requests)
- `Restart` (explicit restart semantics; equivalent to Replace with StartFrom::Explicit)

For v1, implement `Replace` + `IgnoreIfActive` if needed.

---

## 5. API Surface

### 5.1 AnimationRequest
```rust
pub struct AnimationRequest {
    pub property: AnimProp,
    pub from: StartFrom,
    pub to: f32,
    pub duration_ms: u32,
    pub easing: Easing,         // v1: Linear only; extend later
    pub policy: ReplacePolicy,  // v1 default: Replace
}
```

### 5.2 Build-time request collection
`BuildCtx` collects requests each build:

```rust
impl<S> BuildCtx<S> {
    pub fn anim(&mut self) -> AnimCtx<'_, S>;
}
```

`AnimCtx` methods:
- `request(req)` targets current node
- `request_for(target, req)` targets explicit node

No string keys.

### 5.3 Sampling in View
`View` exposes sampled values:

```rust
impl<'a, S> View<'a, S> {
    pub fn anim(&self, id: StableId, prop: AnimProp) -> f32;
}
```

Note:
- returns a value (no Option) because unknown animations resolve to defaults (see §6).
- defaults are deterministic.

---

## 6. Defaults and Missing Animations

If a property has no active animation and no stored value, the runtime returns a deterministic default:
- opacity: 1.0
- transforms: 0.0 (translate/rotation), 1.0 (scale)
- generic driver `t`: 0.0

This removes `Option` in typical usage.

---

## 7. Determinism and Hashing

### 7.1 Animation values must not structural-dirty the tree
Animation outputs are runtime-evaluated values that should:
- not affect structural identity hashes,
- not force layout invalidation unless explicitly animating layout-affecting properties.

The runtime should route animation values into:
- paint inputs hashes (localized),
- transform/layer state (preferred for opacity/transform).

### 7.2 Quantization for hashing
When animation values contribute to paint hashes, quantize deterministically:

```rust
fn quantize_f32(x: f32) -> u16 { /* fixed-point */ }
```

Use quantized values for hashing to avoid floating noise.

---

## 8. Ergonomic Pattern: Triggering a Pulse Without `pulse_generation`

Example: pulse opacity whenever the counter increments.

```rust
fn build(&self, ctx: &mut BuildCtx<AppState>, view: &View<AppState>) -> Node {
    let vm = view.select::<CounterVM>();

    // When state changes, build re-runs; request replaces existing animation.
    ctx.anim().request(AnimationRequest {
        property: AnimProp::opacity(),
        from: StartFrom::Current,
        to: 0.2,
        duration_ms: 250,
        easing: Easing::Linear,
        policy: ReplacePolicy::Replace,
    });

    // Use sampled value
    let opacity = view.anim(ctx.current_id(), AnimProp::opacity());

    // Render using opacity...
}
```

Key points:
- No unique key strings.
- No `pulse_generation`.
- No explicit author-chosen stable id for the common case.
- Runtime deterministically replaces the prior animation each time the request is issued.

If “only trigger when value changed” is desired, add `ctx.anim().request_on_change(dep_hash, req)` as a future convenience, but it is not required for correctness.

---

## 9. Compatibility Notes

### Migration from string-key requests
- Remove `AnimationRequest.key: String`
- Replace dedupe `HashSet<String>` with `(StableId, AnimProp)` map
- Replace `from: vm.anim_val` with `StartFrom::Current`

### Migration from `NodeId::explicit`
- Replace explicit node IDs in animation APIs with `StableId`.
- Continue supporting explicit keys at the authoring layer where stable identity must be specified (lists/reorder).

---

## 10. Summary

This refinement:
- removes “unique key” hacks and confusing counters like `pulse_generation`,
- makes animation semantics deterministic and composable,
- ensures stable targeting without author-managed IDs for typical cases,
- preserves the retained pipeline by keeping animation dirtiness localized.

v2 can extend this model with channels/priorities or blending, but v1 should ship with the simplest deterministic semantics described here.
