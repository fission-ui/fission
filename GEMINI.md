# Fission Framework - Project Knowledge Base (`GEMINI.md`)

**Last Updated:** December 15, 2025
**Scope:** Architecture, Design Principles, Implementation Details, and Development Workflows.

---

## 1. Project Vision & Core Principles

Fission is a next-generation UI framework built on the principle that **UI is a pure function of state**, enforced rigorously through architectural constraints. It aims to solve the "correctness" and "testability" problems in modern UI development.

### Core Principles
1.  **Determinism First:** Given the same input state, the framework *must* produce the exact same pixel output and internal state, regardless of platform, time (mocked), or previous history.
2.  **UI = f(State):** The UI is rebuilt from scratch (logically) whenever state changes. There is no manual DOM manipulation.
3.  **Separation of Concerns:**
    *   **Authoring Layer:** Expressive, open-world API for developers.
    *   **Core Runtime:** Closed-world, platform-agnostic engine handling layout, diffing, and state.
    *   **Shell:** Thin, platform-specific adapter (Windowing, GPU context, Media decoding).
    *   **Renderer:** Pure function turning Display Lists into pixels (Skia).
4.  **Zero-Closure Tree:** The widget tree contains *data*, not behavior. Event handlers are registered alongside the tree, not embedded within it as closures.
5.  **Owned Time:** The framework owns the clock. Animations and physics are deterministic functions of this explicit clock, enabling perfect frame-by-frame replay and testing.

---

## 2. Architecture Overview

The system is divided into four distinct layers:

### Layer 1: Authoring (`crates/authoring`)
*   **Role:** The "Frontend" API used by app developers.
*   **Key Traits:** `Widget<S>`, `View<S>`, `Selector<S>`.
*   **Output:** Produces a `Node` tree (high-level description).
*   **State:** Reads `AppState` (User) and `RuntimeState` (Framework) via `View`.

### Layer 2: Core (`crates/core`)
*   **Role:** The "Brain". Handles logic, diffing, layout, and event routing.
*   **Key Components:**
    *   **Lowering:** Compiles `Node` tree -> `CoreIR` (Ops).
    *   **Runtime:** Manages `AppState`, `RuntimeState` (Scroll, Focus, Anim), and dispatches `Actions`.
*   **Layout:** Computes geometry using a constraint-based layout engine (Flutter-style).
    *   **Diffing:** Compares `CoreIR` trees to optimize updates (Hybrid Retained Mode).
*   **Data:** `CoreIR` is a flat list of `Op`s (Layout, Paint, Semantics).

### Layer 3: Shell (`crates/shell`)
*   **Role:** The "Body". Interfaces with the OS.
*   **Responsibilities:**
    *   Creates Windows/Surfaces (`winit`, `softbuffer`).
    *   Captures OS Events (Mouse, Keyboard, Window).
    *   Provides Media Backends (Video decoding, Audio).
    *   Runs the Event Loop.

### Layer 4: Renderer (`crates/rendering`)
*   **Role:** The "Painter".
*   **Input:** `DisplayList` (Platform-agnostic drawing commands).
*   **Implementation:** `fission-render-vello` is the maintained GPU renderer backend.
*   **Policy:** All rendering must be anti-aliased and resolution-independent.

---

## 3. Core Concepts & Implementation

### 3.1 Authoring: The `Widget` Trait
Widgets are pure data structs. They implement `build` to compose other widgets or primitives.

```rust
impl Widget<S> for MyWidget {
    fn build(&self, ctx: &mut BuildCtx<S>, view: &View<S>) -> Node {
        let val = view.select::<MySelector>();
        Button {
            on_press: Some(ctx.bind(MyAction, on_action)),
            child: Some(Box::new(Text { ... }.into())),
            ...Default::default()
        }.into()
    }
}
```

### 3.2 Lowering: `Node` -> `CoreIR`
The `Lower` trait compiles high-level `Node`s into low-level `CoreIR` operations (`Op`).
*   **Primitives:** `Button`, `Text`, `Row` implement `Lower` directly.
*   **Custom:** `Node::Custom` uses `LowerDyn` to emit arbitrary Ops (escape hatch).
*   **Result:** A list of `LayoutOp` (Box, Flex, Scroll), `PaintOp` (Rect, Text, Image), and `Semantics` (Roles, Actions).

### 3.3 The Action System
*   **Action:** A pure data struct deriving `Action`.
*   **Binding:** `ctx.bind(Action, Handler)` registers the handler in the `ActionRegistry`.
*   **Envelope:** The tree stores an `ActionEnvelope` (ID + Payload), not the closure.
*   **Dispatch:** The Runtime receives the envelope, looks up the handler in the registry, and invokes it with mutable access to `AppState`.

### 3.4 Layout Engine
*   **Backing:** Constraint-based layout engine (Flutter-style).
*   **Integration:** `fission-layout` consumes `LayoutOp` directly.
*   **Text:** `TextMeasurer` trait abstracts platform text sizing (implementation-dependent).
*   **Scroll:** Implemented via unbounded constraints in the scroll axis + clamping in the runtime.

### 3.5 Rendering
*   **Display List:** A serializable list of `DisplayOp` (DrawRect, DrawText, Clip, Translate).
*   **Backend:** `SkiaRenderer` iterates the Display List and issues Skia canvas calls.
*   **Text:** Renders using Skia's `FontMgr` and system fonts.

---

## 4. Key Systems Status

| System | Status | Implementation Details |
| :--- | :--- | :--- |
| **Animation** | ✅ Beta | `Runtime` owns clock. `AnimationRequest` creates `ActiveAnimation`. Values interpolated per frame. |
| **Scrolling** | ✅ Beta | `LayoutOp::Scroll`. `RuntimeState` stores offsets. Input (Wheel/Drag) updates offset. |
| **Video** | ✅ Beta | `VideoBackend` trait (Platform vs Mock). `Runtime` tracks state (Play/Pause). Shell compositing via Layers. |
| **Text Input** | ✅ Alpha | `TextInput` widget. Runtime handles `Char`/`Backspace`. Static cursor. No IME composition yet. |
| **Layout** | ✅ Stable | Constraint-based sizing. Intrinsics + padding/border support. |
| **Theming** | 🚧 Planned | `fission-theme` crate exists. Tokens defined. Logic for inheritance pending. |
| **I18n** | 🚧 Planned | `fission-i18n` crate exists. Basic registry pending. |

---

## 5. Directory Structure

```
/
├── crates/
│   ├── authoring/
│   │   ├── fission/          (Facade crate)
│   │   ├── fission-widgets/  (High-level widgets)
│   │   └── fission-macros/   (Derive macros)
│   ├── core/
│   │   ├── fission-core/     (Runtime, Node, Lowering, Env)
│   │   ├── fission-ir/       (Ops, IDs, Semantics)
│   │   ├── fission-layout/   (constraint layout engine)
│   │   ├── fission-theme/    (Token definitions)
│   │   └── fission-semantics/(Accessibility types)
│   ├── rendering/
│   │   ├── fission-render/   (DisplayList, Traits)
│   │   └── fission-render-vello/ (Vello GPU backend)
│   ├── shell/
│   │   ├── fission-shell/    (Traits: Platform, VideoBackend)
│   │   └── fission-shell-desktop/ (Winit + Softbuffer implementation)
│   └── tools/
│       └── fission-test/     (Headless harness)
├── examples/
│   └── counter/              (Reference app)
└── docs/                     (Architecture specs)
```

---

## 6. Development Workflows

### Adding a New Primitive Widget
1.  **Define Struct:** Create `pub struct MyWidget { ... }` in `crates/core/fission-core/src/ui/widgets/`.
2.  **Implement `Lower`:** Implement `fn lower(...)` to emit `LayoutOp`s and `PaintOp`s.
3.  **Register Node:** Add variant to `Node` enum in `crates/core/fission-core/src/ui/node.rs`.
4.  **Export:** Add to `crates/core/fission-core/src/ui/mod.rs` and `lib.rs`.

### Adding a High-Level Widget
1.  **Define Struct:** Create struct in `crates/authoring/fission-widgets/src/`.
2.  **Implement `Widget`:** Implement `build` to return a `Node` composed of existing primitives.

### Running Tests (Mandatory)
*   **Test Command:** `cargo test --no-fail-fast --quiet -- --nocapture`
*   **Policy:** A **Test-Driven Development (TDD)** approach is strictly required. Tests must be written *before* or *alongside* implementation. All tests must pass before committing changes.
*   **Unit Tests:** `cargo test -p fission-core` for runtime logic.
*   **Invariant Tests:** `cargo test -p fission-layout` for layout behavior.
*   **Reference App:** `cargo run -p counter` to verify integration.

### Code Style & Conventions
*   **No Unwraps in Runtime:** Propagate `Result`. Panics only for invariant violations (e.g. infinite loops).
*   **Explicit IDs:** Use `WidgetNodeId` for stable identity in animations/media.
*   **No Interior Mutability:** Widgets are immutable. State is mutated only in Reducers.

---
**Note to Agent:** When modifying the codebase, strictly adhere to the **"Zero-Closure"** and **"Deterministic"** mandates. Always check `docs/` for the latest specific subsystem specs.
