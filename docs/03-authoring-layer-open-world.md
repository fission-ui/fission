# Authoring API Spec (v1): Widget, View::select, and Lowering (with Custom Lowering)

This document specifies the **v1 authoring model** for the framework, focusing on:

1. `Widget<S>`: a Flutter-like composition trait that **builds** UI.
2. `View<S>::select::<VM>()`: a type-safe selector system for scalable state access.
3. `Lower`: a lowering trait that compiles an authored `Node` tree into Core IR deterministically.
4. `Node::Custom`: an **escape hatch** enabling developers to emit arbitrary Core IR for advanced widgets (e.g., a star-shaped button).

This spec explicitly **excludes** any `Provide<T>` / provider mechanism. All inputs are carried through `View<S>`.

---

## 1. Goals and Non-Goals

### Goals
- Scale to large applications (many modules/files/crates).
- Keep UI as a pure function of explicit inputs:  
  **UI = f(AppState, RuntimeState, Env)**.
- Avoid closures in the widget tree; action handling is registered via `BuildCtx`.
- Avoid trait objects in the authored tree (no `dyn Widget` trees).
- Make lowering automatic for the common case.
- Allow **optional** custom lowering for advanced cases without changing core primitives.

### Non-Goals (v1)
- Provider/DI style scoping (`Provide<T>`).
- Incremental partial rebuild heuristics (v1 rebuilds full authored tree per frame).
- Complex binding expression languages (state access is via `select` and normal Rust computation).

---

## 2. Key Types and Responsibilities

### 2.1 `AppState`
User-defined persistent state, serializable and deterministic.

```rust
pub trait AppState: Default + Clone + 'static {}
```

### 2.2 `RuntimeState`
Framework-owned ephemeral state:
- hover / pressed / focus
- pointer capture
- animation clocks / scroll offsets (future)

### 2.3 `Env`
Explicit environment inputs:
- DPI / device pixel ratio
- platform id (android/ios/web/desktop)
- accessibility preferences
- theme pack selection (or resolved theme)
- i18n registry + locale (v1 i18n)

### 2.4 `View<S>`
Read-only view over all inputs required to build UI.

```rust
pub struct View<'a, S> {
    pub state: &'a S,
    pub runtime: &'a RuntimeState,
    pub env: &'a Env,
    pub theme: &'a Theme,
    pub i18n: &'a I18nRegistry,
}
```

---

## 3. `Selector` and `View::select`

### 3.1 Selector trait
`Selector<S>` ensures `select()` always returns a value.

```rust
pub trait Selector<S> {
    type Output;
    fn select(view: &View<S>) -> Self::Output;
}
```

### 3.2 `View::select`
```rust
impl<'a, S> View<'a, S> {
    pub fn select<T: Selector<S>>(&self) -> T::Output {
        T::select(self)
    }
}
```

No `Option`, no runtime lookup failure.

---

## 4. `BuildCtx<S>` and Action Binding

`BuildCtx<S>` exists to register action handlers and other build-time services.
It is *not* the mechanism for reading app state.

```rust
Button {
    on_press: Some(with_reducer!(ctx, Increment, on_increment)),
    ..Default::default()
}
```

`bind` registers the handler deterministically and returns a pure `ActionEnvelope` stored in the tree.

---

## 5. `Widget<S>`: Composition, Not Rendering

### 5.1 Widget trait
```rust
pub trait Widget<S: AppState> {
    fn build(&self, ctx: &mut BuildCtx<S>, view: &View<S>) -> impl IntoWidget<S>;
}
```

### 5.2 Why `build()` returns `Node`
Returning `Node` avoids `dyn Widget` trees and object-safety issues.
The authored UI is represented by a single erased sum type (`Node`), suitable for deterministic lowering.

---

## 6. `Node`: the Authoring Tree Carrier

`Node` is the uniform tree carrier produced by widgets.

```rust
pub enum Node {
    Row(Row),
    Text(Text),
    Button(Button),
    Padding(Padding),
    Container(Container),
    // ...
    Custom(CustomNode), // escape hatch
}
```

Custom widgets typically “compile away” into primitives by returning `Node` composed of primitive variants.
For advanced cases, a widget may return `Node::Custom(...)` to supply its own lowering.

---

## 7. `Lower`: Compiling `Node` → Core IR

### 7.1 Lower trait
```rust
pub trait Lower {
    fn lower(&self, cx: &mut LoweringContext) -> CoreIrNode;
}
```

### 7.2 Who implements `Lower`?
- Primitive node types (`Row`, `Text`, `Button`, etc.) implement `Lower`.
- `Node` implements `Lower` by delegating to the contained variant.
- `CustomNode` participates via `LowerDyn` (see §8).

Lowering happens **after** the widget tree is built, and it walks the entire `Node` tree.

---

## 8. Custom Lowering Escape Hatch: `Node::Custom`

### 8.1 Motivation
Some widgets cannot be expressed cleanly as a composition of existing primitives:
- custom shapes (star, hex, irregular path)
- specialized hit-testing and semantics regions
- bespoke paint operations

To support these without bloating the primitive set, we introduce `Node::Custom`.

### 8.2 `CustomNode`
A `CustomNode` carries:
- a stable debug tag,
- a user-supplied lowerer implementing an object-safe trait.

```rust
pub struct CustomNode {
    pub debug_tag: &'static str,
    pub lowerer: std::sync::Arc<dyn LowerDyn>,
}
```

### 8.3 `LowerDyn` trait (object-safe)
`LowerDyn` is a deliberately small surface:
- object-safe,
- deterministic,
- no closures in the authored tree,
- no dependency on external state.

```rust
pub trait LowerDyn: Send + Sync {
    fn lower_dyn(&self, cx: &mut LoweringContext) -> CoreIrNode;

    /// Optional: stable key used for caching/debugging.
    /// Must be deterministic for identical inputs.
    fn stable_key(&self) -> u64 { 0 }
}
```

### 8.4 How `Node::Custom` lowers
`Node::lower(cx)` delegates:

- `Node::Row(r) => r.lower(cx)`
- ...
- `Node::Custom(c) => c.lowerer.lower_dyn(cx)`

This means custom nodes lower at their position in the tree and can emit arbitrary Core IR.

---

## 9. Determinism Constraints Checklist

Any implementation of `Widget::build`, `Lower`, or `LowerDyn` must obey these constraints.

### Forbidden Inputs
- Wall-clock time (must use Core-owned clock via `RuntimeState/Env`).
- Randomness without explicit seeded input.
- OS locale/font fallback, filesystem, network, environment variables.
- HashMap iteration order unless keys are sorted deterministically.

### Allowed Inputs
- `View<S>` (state + runtime + theme + i18n + env) — explicit and pinned.
- `BuildCtx<S>` for action binding/registration.
- `LoweringContext` with pinned resources (fonts, rounding rules, etc.)

### Output Requirements
- Stable child ordering.
- Stable paint ordering.
- Canonical rounding and coordinate normalization.
- No hidden side effects during build/lower.

### Debuggability (recommended)
- Provide stable `debug_tag` for `CustomNode`.
- Prefer stable keys or stable metadata for caching/inspection.

---

## 10. Concrete Example: `StarButton` (Path + Hit Test + Semantics + Action)

This example demonstrates a custom widget that:
- binds an action handler using `BuildCtx`,
- renders a star-shaped button via a path paint op,
- participates in hit-testing,
- exposes semantics (role + label + press action),
- lowers via `Node::Custom` without adding new primitives.

Core IR operation names below are illustrative.

### 10.1 Action and handler
```rust
#[fission_action]
pub struct StarPressed;

fn on_star_pressed(state: &mut AppState, _: StarPressed) {
    state.star_count += 1;
}
```

### 10.2 The widget (authoring)
```rust
pub struct StarButton {
    pub id: WidgetNodeId,
    pub label: String,
}

impl Widget<AppState> for StarButton {
    fn build(&self, ctx: &mut BuildCtx<AppState>, view: &View<AppState>) -> impl IntoWidget<AppState> {
        // Bind handler at authoring site (ergonomic)
        let on_press = with_reducer!(ctx, StarPressed, on_star_pressed);

        // Resolve style from theme deterministically
        let style = view.theme.components.button.primary;

        Node::Custom(CustomNode {
            debug_tag: "StarButton",
            lowerer: std::sync::Arc::new(StarButtonLower {
                id: self.id,
                label: self.label.clone(),
                on_press,
                style,
            }),
        })
    }
}
```

### 10.3 The lowerer (custom Core IR emission)
```rust
pub struct StarButtonLower {
    pub id: WidgetNodeId,
    pub label: String,
    pub on_press: ActionEnvelope,
    pub style: ButtonStyle, // pure data (resolved or partially resolved)
}

impl LowerDyn for StarButtonLower {
    fn stable_key(&self) -> u64 {
        // Must be deterministic. Hash only deterministic fields.
        // (Pseudo-code) hash(id, label, on_press.id, style.version_id)
        0
    }

    fn lower_dyn(&self, cx: &mut LoweringContext) -> CoreIrNode {
        // 1) Layout: define a sized box (or constraints)
        let size = cx.layout.box_fixed(56.0, 56.0);

        // 2) Build a star path in local coordinates (deterministic math)
        let path = cx.paint.path_star(
            /* center */ (28.0, 28.0),
            /* points */ 5,
            /* inner */ 10.0,
            /* outer */ 24.0,
        );

        // 3) Paint: fill star with theme-derived color
        let paint = cx.paint.fill_path(path, self.style.background_color);

        // 4) Hit-test: use the same path as the hit region
        let hit = cx.input.hit_test_path(self.id, path);

        // 5) Semantics: role=Button, label, and press action
        let sem = cx.semantics.node(self.id)
            .role_button()
            .label(&self.label)
            .action_press(self.on_press.clone());

        // 6) Compose into a Core IR subtree
        cx.core.compose([size, paint, hit, sem])
    }
}
```

---

## 11. Summary

- `Widget<S>::build` composes UI and returns `Node`.
- `Node` is the uniform tree carrier for authored UI.
- `Lower` compiles `Node` to Core IR automatically by walking the entire tree.
- `Node::Custom` + `LowerDyn` enables optional, advanced custom lowering.
- Determinism is preserved by strict input/output constraints and explicit runtime inputs.
