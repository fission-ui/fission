# Fission

[![Crates.io](https://img.shields.io/crates/v/fission.svg)](https://crates.io/crates/fission)
[![docs.rs](https://docs.rs/fission/badge.svg)](https://docs.rs/fission)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![CI](https://github.com/worka-ai/fission/actions/workflows/ci.yml/badge.svg)](https://github.com/worka-ai/fission/actions/workflows/ci.yml)

Cross-platform, GPU-accelerated UI framework for Rust. Fission uses a Flutter-inspired widget architecture with a deterministic state management model built on serializable actions and reducers. Rendering is powered by [Vello](https://github.com/linebender/vello) and [wgpu](https://wgpu.rs/), delivering high-performance 2D graphics on every platform. Desktop (macOS, Linux, Windows) is fully supported today; iOS simulator, Android emulator, and Web (WASM) smoke paths are now runnable as well.

---

## Table of contents

- [Quick start](#quick-start)
- [Project scaffolding](#project-scaffolding)
- [Platform smoke tests](#platform-smoke-tests)
- [Architecture](#architecture)
- [Deterministic state management](#deterministic-state-management)
- [Effects system](#effects-system)
- [Built-in widgets](#built-in-widgets)
- [Theming](#theming)
- [Internationalisation](#internationalisation)
- [Accessibility and semantics](#accessibility-and-semantics)
- [Animations](#animations)
- [Icons](#icons)
- [Custom render objects](#custom-render-objects)
- [Testing](#testing)
- [Diagnostics](#diagnostics)
- [Platform support](#platform-support)
- [Project structure](#project-structure)
- [Building from source](#building-from-source)
- [Running examples](#running-examples)
- [Crate map](#crate-map)
- [Contributing](#contributing)
- [License](#license)

---

## Quick start

Add Fission to your project:

```sh
cargo add fission
```

A minimal application:

```rust
use fission::prelude::*;

// 1. Define your application state
#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
struct MyState {
    count: i32,
}
impl AppState for MyState {}

// 2. Define an action
#[derive(Action, Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
struct Increment;

// 3. Build your widget tree
struct MyApp;

impl Widget<MyState> for MyApp {
    fn build(&self, ctx: &mut BuildCtx<MyState>, view: &View<MyState>) -> Node {
        Column {
            children: vec![
                Text {
                    content: TextContent::Literal(format!("Count: {}", view.state.count)),
                    font_size: Some(24.0),
                    ..Default::default()
                }
                .into(),
                Button {
                    on_press: Some(ctx.bind(Increment, |s: &mut MyState, _: Increment, _| {
                        s.count += 1;
                    })),
                    child: Some(Box::new(
                        Text::new("Increment").into(),
                    )),
                    ..Default::default()
                }
                .into(),
            ],
            ..Default::default()
        }
        .into()
    }
}

// 4. Launch
fn main() -> anyhow::Result<()> {
    DesktopApp::new(MyApp).run()
}
```

## Project scaffolding

Fission now ships a first-party scaffolding CLI for the basic project lifecycle:

```sh
# Standalone binary
fission init my-app

# Cargo subcommand alias
cargo fission add-target web ios android --project-dir my-app
```

The CLI currently does three things:

- creates a runnable desktop app skeleton
- scaffolds platform folders for `windows`, `macos`, `linux`, `web`, `ios`, and `android`
- records target state in `fission.toml`

Current status:

- desktop targets are runnable today through `DesktopApp`
- iOS now has a verified simulator run path both through `examples/mobile-smoke/` and through CLI-generated apps after `fission add-target ios`
- Android now has a verified emulator run path through `examples/mobile-smoke/` and through CLI-generated apps after `fission add-target android`
- web/WASM now has a checked-in `web-smoke` example and CLI-generated browser host projects after `fission add-target web`

If you are developing against a local Fission checkout, use:

```sh
fission init my-app --local-path /path/to/fission
```

That generates path dependencies so the new app tracks your local workspace instead of crates.io.

More detail lives in:

- `docs/cli-and-targets.md`
- `docs/platform-smoke-tests.md`

## Platform smoke tests

The current reproducible smoke path is:

- desktop preview: `cargo run -p mobile-smoke`
- iOS simulator smoke: `FISSION_TEST_CONTROL_PORT=48711 ./examples/mobile-smoke/platforms/ios/run-sim.sh`
- Android emulator smoke: `FISSION_TEST_CONTROL_PORT=48761 ./examples/mobile-smoke/platforms/android/run-emulator.sh`
- browser smoke: `./examples/web-smoke/platforms/web/run-browser.sh`

Install the extra Rust targets first:

```sh
rustup target add aarch64-apple-ios aarch64-apple-ios-sim aarch64-linux-android wasm32-unknown-unknown
```

On macOS, the Android check also needs the SDK + NDK environment:

```sh
export ANDROID_HOME="$HOME/Library/Android/sdk"
export ANDROID_NDK="$ANDROID_HOME/ndk/24.0.8215888"
export ANDROID_TOOLCHAIN="$ANDROID_NDK/toolchains/llvm/prebuilt/darwin-x86_64/bin"
export CC_aarch64_linux_android="$ANDROID_TOOLCHAIN/aarch64-linux-android24-clang"
export AR_aarch64_linux_android="$ANDROID_TOOLCHAIN/llvm-ar"
```

If your NDK uses a different host prebuilt directory, replace `darwin-x86_64` with the correct value for your machine.

Android emulator notes:

- `run-emulator.sh` launches a visible emulator when it has to boot a new AVD
- set `ANDROID_EMULATOR_HEADLESS=1` for CI/background runs
- set `ANDROID_EMULATOR_RESTART=1` if an old hidden emulator is already running and you want the script to relaunch it visibly
- the shell forces `WGPU_BACKEND=gl` on Android when `WGPU_BACKEND` is unset; this avoids the emulator's unstable Vulkan/SwiftShader path
- set `WGPU_BACKEND=vulkan` explicitly only if you want to test a real-device Vulkan path yourself
- when `FISSION_TEST_CONTROL_PORT` is set on Android, the shell keeps the event loop polling so test-control commands can wake and drain reliably inside the emulator

iOS simulator note:

- CoreSimulator still lacks `DownlevelFlags(INDIRECT_EXECUTION)`, so the shell falls back to the shared software renderer automatically on the simulator

Web/WASM prerequisites are:

```sh
rustup target add wasm32-unknown-unknown
cargo install wasm-pack
```

The browser smoke script builds the wasm package and serves:

- `http://127.0.0.1:8123/examples/web-smoke/platforms/web/`

It does not auto-open a tab unless you set `FISSION_WEB_OPEN=1`.

## Architecture

Every frame follows a deterministic pipeline:

```
Widget::build() --> Node tree --> Lower to IR --> Layout --> Paint --> Render (Vello/wgpu)
```

| Stage | What happens |
|---|---|
| **Build** | Widgets produce a declarative `Node` tree from the current state. This is a pure function -- no side effects. |
| **Lower** | The `Node` tree is lowered into an intermediate representation (IR) -- a flat graph of layout, paint, and semantic operations. |
| **Layout** | The constraint-based layout engine (flexbox + box model) resolves sizes and positions for every IR node. |
| **Paint** | Paint operations (rectangles, text runs, images, shadows) are emitted for each visible node. |
| **Render** | Vello tessellates the paint ops into GPU draw calls and wgpu submits them. |

The key insight: widgets never directly mutate pixels. They declare *what* should be shown, and the pipeline handles *how*. This separation enables structural diffing between frames, headless testing, and deterministic replay.

### The Widget trait

```rust
pub trait Widget<S: AppState> {
    fn build(&self, ctx: &mut BuildCtx<S>, view: &View<S>) -> Node;
}
```

`build()` is called once per frame. Implementations must be pure -- all side-effects (action binding, portal registration, animation requests) go through `ctx`.

### The LowerDyn trait

For custom render objects that need direct control over the IR:

```rust
pub trait LowerDyn: Send + Sync + Debug {
    fn lower_dyn(&self, cx: &mut LoweringContext) -> NodeId;
    fn stable_key(&self) -> u64 { /* default impl */ }
}
```

## Deterministic state management

State changes flow exclusively through **Actions** and **Reducers**. Actions are strongly-typed, serializable structs. Reducers are pure functions that take `(&mut State, Action, &mut ReducerContext)` and mutate state. No shared mutable state, no callbacks, no implicit side effects.

This makes the UI fully deterministic: given the same state and the same sequence of actions, you get the same output -- guaranteed. This property enables:

- **Time-travel debugging** -- replay any sequence of actions to reproduce a bug
- **Serializable history** -- every state transition is a serializable action
- **Headless testing** -- drive the full widget tree without a GPU

```rust
#[derive(Action, Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
struct Increment;

// Bind an action to a reducer inline
let action = ctx.bind(Increment, |s: &mut CounterState, _, _| {
    s.count += 1;
});

// Or define a named handler
fn on_increment(
    state: &mut CounterState,
    _action: Increment,
    _ctx: &mut ReducerContext<CounterState>,
) {
    state.value += 1;
}
let action = ctx.bind(Increment, on_increment as Handler<CounterState, Increment>);
```

### Selectors

Selectors extract derived data from state, avoiding redundant computation:

```rust
struct CounterVM {
    label: String,
    is_even: bool,
}

impl Selector<CounterState> for CounterVM {
    type Output = CounterVM;
    fn select(view: &View<CounterState>) -> Self::Output {
        CounterVM {
            label: format!("Count: {}", view.state.value),
            is_even: view.state.value % 2 == 0,
        }
    }
}

// In build():
let vm = view.select::<CounterVM>();
```

## Effects system

Reducers must be pure -- they cannot perform I/O. When async work is needed, reducers emit **Effects** through the `ReducerContext`. The platform executor fulfills the effect outside the deterministic core and dispatches the result back as a bound callback action.

Built-in system effects:

| Effect | Description |
|---|---|
| `HttpGet` | Perform an HTTP GET request with custom headers |
| `FileRead` | Read a file from the local filesystem |
| `Alert` | Show a native alert dialog |
| `OpenUrl` | Open a URL in the system browser or in-app browser sheet |
| `Authenticate` | Initiate an OAuth / secure authentication session |
| `Cancel` | Cancel a previously issued effect by request ID |
| `ReleaseResource` | Free a platform-managed resource handle |

For app-specific effects, use `Effect::App` with an opaque byte payload.

```rust
fn fetch_data(state: &mut MyState, _action: FetchTodos, ctx: &mut ReducerContext<MyState>) {
    ctx.effects.http_get("https://api.example.com/todos")
        .on_ok(ctx.effects.bind(TodosLoaded, handle_loaded as fn(&mut MyState, TodosLoaded, _)))
        .on_err(ctx.effects.bind(FetchError, handle_error as fn(&mut MyState, FetchError, _)));
}
```

## Built-in widgets

### Layout
| Widget | Description |
|---|---|
| `Row` / `HStack` | Horizontal flex container |
| `Column` / `VStack` | Vertical flex container |
| `Container` | Box with padding, sizing, and background |
| `Scroll` | Scrollable viewport with optional scrollbar |
| `ZStack` | Overlapping children stacked on the z-axis |
| `Positioned` | Absolutely positioned child within a `ZStack` |
| `Align` | Aligns a single child within available space |
| `Grid` | CSS Grid-style layout with rows and columns |
| `Spacer` | Flexible or fixed-size empty space |
| `SplitView` | Resizable split pane (horizontal or vertical) |
| `Wrap` | Flow layout that wraps children to the next line |

### Input
| Widget | Description |
|---|---|
| `Button` | Clickable button with Filled, Outline, and Ghost variants |
| `TextInput` | Single-line editable text field with placeholder and change events |
| `Checkbox` | Toggle with checked/unchecked state and optional label |
| `Switch` | On/off toggle switch |
| `Radio` | Mutually exclusive radio button within a group |
| `Slider` | Continuous range input with min, max, and step |
| `GestureDetector` | Low-level pointer event handler (tap, drag, hover) |
| `Combobox` | Text input with a filterable dropdown list |
| `Select` | Dropdown selection from a list of options |

### Display
| Widget | Description |
|---|---|
| `Text` | Styled text label with literal or i18n key content |
| `Icon` | SVG icon from the bundled Material Design icon set |
| `Image` | Raster or vector image from a path or URL |
| `Video` | Hardware-accelerated video player with playback controls |
| `Badge` | Small status indicator, typically overlaid on another widget |
| `Tag` | Labelled chip for categories, filters, or metadata |
| `Card` | Elevated surface container with rounded corners |
| `Avatar` | Circular image or initials placeholder |
| `Divider` | Horizontal or vertical separator line |
| `Progress` | Determinate or indeterminate progress bar |
| `Skeleton` | Pulsing placeholder for content that is loading |
| `Spinner` | Three-dot animated loading indicator |

### Overlay
| Widget | Description |
|---|---|
| `Modal` | Full-screen dialog overlay with backdrop dimming |
| `Popover` | Anchored floating panel positioned relative to a trigger |
| `Tooltip` | Small informational popup on hover or focus |
| `Menu` | Context or dropdown menu with items and sub-menus |
| `Toast` | Temporary notification that auto-dismisses |
| `Drawer` | Slide-in panel from any edge of the screen |

### Navigation
| Widget | Description |
|---|---|
| `Tabs` | Tabbed container with tab bar and content panels |
| `Accordion` | Collapsible sections with expand/collapse animation |
| `SegmentedControl` | Mutually exclusive segment selector |

### Composition
| Widget | Description |
|---|---|
| `Hero` | Shared-element transition anchor for navigation animations |
| `Portal` | Renders its child into the top-level overlay layer |
| `CustomNode` | Escape hatch for custom rendering via `LowerDyn` |

## Theming

Fission's theme system is built on **design tokens** following the Material Design 3 token architecture:

- **Colors** -- semantic palette (primary, secondary, surface, background, error, border, text) with `on_*` counterparts for content on each surface
- **Spacing** -- consistent spacing scale (xs, sm, md, lg, xl, xxl, xxxl)
- **Typography** -- font families, sizes, weights, and line heights for display, headline, title, body, label, and caption styles
- **Corner radii** -- none, xs, sm, md, lg, xl, full
- **Elevations** -- box shadow presets for depth levels 0 through 5

Themes also include per-component overrides (button colors, input borders, card shadows, etc.).

```rust
// Use the default light theme
let light = Theme::default();

// Switch to dark mode
let dark = Theme::dark();

// Access tokens in a widget
let primary = view.env.theme.tokens.colors.primary;
let padding = view.env.theme.tokens.spacing.md;
```

## Internationalisation

The `fission-i18n` crate provides a registry-based translation system with BCP 47 locale identifiers:

```rust
use fission::i18n::{I18nRegistry, TranslationBundle, Locale};

let mut registry = I18nRegistry::new();
registry.add_bundle(TranslationBundle {
    locale: Locale::from("en-US"),
    messages: [("greeting".into(), "Hello!".into())].into(),
});
registry.add_bundle(TranslationBundle {
    locale: Locale::from("ja-JP"),
    messages: [("greeting".into(), "こんにちは!".into())].into(),
});

let msg = registry.get(&Locale::from("ja-JP"), "greeting");
// => Some("こんにちは!")
```

In widgets, use `TextContent::Key("greeting")` to look up translated strings at render time. The active locale is stored in the environment (`Env`) and flows through the widget tree automatically.

## Accessibility and semantics

Every built-in widget emits **Semantics** metadata: role, label, value, and focusable state. The semantic tree is extracted after layout and exposed to platform accessibility APIs.

Supported semantic roles:

`Button`, `Text`, `TextInput`, `Image`, `Checkbox`, `Switch`, `Dialog`, `Slider`, `Input`, `List`

Focus traversal is managed by the runtime -- Tab / Shift-Tab cycles through focusable nodes in tree order. Screen reader support is on the roadmap via platform accessibility bridges (NSAccessibility on macOS, AT-SPI on Linux, UIA on Windows).

The `GetTree` test command returns the full semantic tree as a flat list of `SemanticNode` values, enabling automated accessibility assertions in tests.

## Animations

Animations are time-based and deterministic. Request an animation during `build()` and read the interpolated value on the next frame:

```rust
ctx.anim_for(widget_id).request(AnimationRequest {
    property: AnimationPropertyId::Opacity,
    from: AnimationStartValue::Explicit(0.0),
    to: 1.0,
    duration_ms: 300,
    repeat: false,
    delay_ms: 0,
});

// Read the current animated value
let opacity = view.animation_value(widget_id, &AnimationPropertyId::Opacity);
```

### Features

- **Custom properties** -- `AnimationPropertyId::custom("pulse_intensity")` for any numeric value
- **Repeating animations** -- `repeat: true` for spinners, pulsing indicators, and looping effects
- **Staggered animations** -- use `delay_ms` to offset animations for wave effects (see `Spinner`)
- **Start values** -- `AnimationStartValue::Current` continues from the current value; `Explicit(v)` starts from a fixed value
- **Deterministic** -- animations are driven by elapsed time, not wall-clock; identical inputs produce identical outputs

## Icons

Material Design icons are bundled in the `fission-icons` crate, generated at build time from SVG data. No external assets, no checkout steps, no network fetches required.

```rust
use fission::icons::material;

Icon::svg(material::action::search::regular()).size(24.0)
```

Icons are organized by Material Design category: action, alert, av, communication, content, editor, file, hardware, image, maps, navigation, notification, social, toggle. Browse all available icons using the `icons_gallery` example.

## Custom render objects

For widgets that need direct control over the rendering pipeline -- code editors, charts, visualizations, games -- implement the `LowerDyn` trait on a custom struct and wrap it in a `CustomNode`:

```rust
use std::sync::Arc;

impl LowerDyn for MyCustomRenderer {
    fn lower_dyn(&self, cx: &mut LoweringContext) -> NodeId {
        let paint = NodeBuilder::new(cx.next_node_id(), Op::Paint(PaintOp::DrawRect {
            fill: Some(Fill { color: IrColor::RED }),
            stroke: None,
            corner_radius: 4.0,
            shadow: None,
        })).build(cx);

        let mut layout = NodeBuilder::new(cx.next_node_id(), Op::Layout(LayoutOp::Box {
            width: Some(200.0), height: Some(100.0),
            min_width: None, max_width: None, min_height: None, max_height: None,
            padding: [0.0; 4],
            flex_grow: 0.0, flex_shrink: 0.0, aspect_ratio: None,
        }));
        layout.add_child(paint);
        layout.build(cx)
    }
}

// Use it in a widget tree
Node::Custom(CustomNode {
    debug_tag: "MyRenderer".into(),
    lowerer: Some(Arc::new(MyCustomRenderer { /* ... */ })),
})
```

The Fission Editor example uses this mechanism for its entire code editing surface, including syntax-highlighted text rendering, cursor painting, and selection highlights.

## Testing

Fission provides two testing approaches:

### Headless testing with TestDriver

Build a widget tree, apply actions, and assert on the resulting node structure without a GPU. Because widgets are pure functions of state, you can test them as regular unit tests.

### Integration testing with LiveTestClient

Connect to a running Fission application over HTTP and drive it programmatically:

```rust
use fission::test_driver::{LiveTestClient, TestCommand};

let client = LiveTestClient::connect(9876).await?;
client.send(TestCommand::TapText { text: "Increment".into() }).await?;
client.send(TestCommand::Screenshot { path: "/tmp/after_click.png".into() }).await?;
let response = client.send(TestCommand::GetText {}).await?;
```

Available test commands:

| Command | Description |
|---|---|
| `Tap { x, y }` | Simulate a pointer tap at pixel coordinates |
| `TapText { text }` | Tap the first visible element containing the given text |
| `TypeText { text }` | Type text into the focused input |
| `PressKey { key, modifiers }` | Send a keyboard event |
| `Scroll { x, y, dx, dy }` | Simulate a scroll gesture |
| `Screenshot { path }` | Capture the current frame to a PNG file |
| `GetText {}` | Return all visible text elements with bounding rects |
| `GetTree {}` | Return the semantic accessibility tree |
| `Wait { ms }` | Wait for the given duration |
| `Pump {}` | Advance one frame |
| `Quit {}` | Close the application |

Launch the application with `FISSION_TEST_CONTROL_PORT=9876` to enable the test server.

GPU screenshot capture enables visual regression testing. Because the framework is deterministic, same state + same actions = same rendered output, guaranteed.

## Diagnostics

Structured diagnostic system covering every stage of the frame lifecycle. Events are categorized and leveled:

**Categories:** Frame, Diff, Layout, Paint, Raster, Input, Semantics, Animation, Media, Invariants, Test

**Levels:** Error, Warn, Info, Debug, Trace

Configure via environment variables:

```sh
FISSION_DIAG_LEVEL=debug FISSION_DIAG_CATEGORIES=Layout,Paint cargo run --example counter
```

Or programmatically:

```rust
use fission::diagnostics::prelude::*;

init_from_env();
```

Output is structured JSON, suitable for piping into analysis tools, dashboards, or log aggregators. File output is supported via `FISSION_DIAG_FILE`.

## Platform support

| Platform | Status |
|----------|--------|
| macOS | Supported |
| Linux | Supported |
| Windows | Supported |
| iOS | Simulator supported, device packaging in progress |
| Android | Emulator supported, device packaging in progress |
| Web (WASM) | Browser smoke path supported |

## Project structure

```
fission/
├── crates/
│   ├── core/
│   │   ├── fission-core/          # Runtime, built-in widgets, actions, reducers, effects
│   │   ├── fission-ir/            # Intermediate representation (node graph)
│   │   ├── fission-layout/        # Constraint-based layout engine
│   │   ├── fission-theme/         # Design tokens and component themes
│   │   ├── fission-i18n/          # Internationalisation
│   │   └── fission-semantics/     # Accessibility roles and semantic types
│   ├── authoring/
│   │   ├── fission/               # Facade crate (the one you depend on)
│   │   ├── fission-widgets/       # Higher-level widgets (Modal, Tabs, etc.)
│   │   ├── fission-macros/        # Derive macros (#[derive(Action)])
│   │   └── fission-icons/         # Material Design icons
│   ├── rendering/
│   │   ├── fission-render/        # Rendering primitives and display list
│   │   └── fission-render-vello/  # Vello/wgpu rendering backend
│   ├── shell/
│   │   ├── fission-shell/         # Shared shell abstractions
│   │   ├── fission-shell-desktop/ # Desktop shell (winit + Vello + wgpu)
│   │   ├── fission-shell-mobile/  # Mobile shell (iOS / Android)
│   │   └── fission-shell-web/     # Web shell (WASM / browser)
│   └── tools/
│       ├── fission-cli/           # `fission` / `cargo fission` scaffolding CLI
│       ├── fission-diagnostics/   # Structured diagnostic logging
│       ├── fission-test/          # Test utilities
│       └── fission-test-driver/   # LiveTestClient and test protocol
├── examples/
│   ├── counter/                   # Minimal counter app
│   ├── inbox/                     # Email client demo
│   ├── editor/                    # VS Code-style code editor
│   ├── widget-gallery/            # Showcase of all built-in widgets
│   ├── icons_gallery/             # Browse all Material Design icons
│   ├── chart-gallery/             # Chart types showcase
│   └── text-lab/                  # Text rendering experiments
└── Cargo.toml                     # Workspace manifest
```

## Building from source

### Prerequisites

- Rust 1.77+ (stable)
- A GPU driver supporting Vulkan, Metal, or DX12 (for wgpu)
- On Linux: `libwayland-dev`, `libxkbcommon-dev`, and X11/Wayland development headers

### Build

```sh
git clone https://github.com/worka-ai/fission.git
cd fission
git submodule update --init --recursive
cargo build
```

### Build in release mode

```sh
cargo build --release
```

## Running examples

```sh
# Minimal counter app
cargo run --example counter

# Email client demo
cargo run --example inbox

# VS Code-style code editor
cargo run --example editor

# Widget gallery
cargo run --example widget-gallery

# Icon browser
cargo run --example icons_gallery

# Chart gallery
cargo run --example chart-gallery

# Text rendering lab
cargo run --example text-lab

# Mobile shell smoke preview on the host
cargo run -p mobile-smoke

# Browser shell smoke preview on the host
cargo run -p web-smoke
```

For the iOS, Android, and browser smoke commands, see `docs/platform-smoke-tests.md`.

## Crate map

| Crate | Description |
|---|---|
| [`fission`](crates/authoring/fission) | Facade crate -- single dependency for applications |
| [`fission-core`](crates/core/fission-core) | Runtime, built-in widgets, actions, reducers, effects, animations |
| [`fission-ir`](crates/core/fission-ir) | Intermediate representation -- the flat node graph between widgets and layout |
| [`fission-layout`](crates/core/fission-layout) | Constraint-based layout engine (flexbox + box model + grid) |
| [`fission-theme`](crates/core/fission-theme) | Design tokens, component themes, dark/light mode |
| [`fission-i18n`](crates/core/fission-i18n) | Internationalisation -- locale registry and string lookups |
| [`fission-semantics`](crates/core/fission-semantics) | Accessibility roles and semantic tree types |
| [`fission-widgets`](crates/authoring/fission-widgets) | Higher-level authoring widgets (Modal, Popover, Tabs, SplitView, etc.) |
| [`fission-macros`](crates/authoring/fission-macros) | Derive macros (`#[derive(Action)]`) |
| [`fission-icons`](crates/authoring/fission-icons) | Material Design icon set, generated from bundled SVGs |
| [`fission-render`](crates/rendering/fission-render) | Rendering primitives -- display list, paint ops, text styles |
| [`fission-render-vello`](crates/rendering/fission-render-vello) | Vello/wgpu rendering backend |
| [`fission-shell`](crates/shell/fission-shell) | Shared shell abstractions (event loop, windowing) |
| [`fission-shell-winit`](crates/shell/fission-shell-winit) | Shared winit + Vello/runtime layer used by desktop, mobile, and browser shells |
| [`fission-shell-desktop`](crates/shell/fission-shell-desktop) | Desktop shell wrapper around the shared winit runtime |
| [`fission-shell-mobile`](crates/shell/fission-shell-mobile) | Mobile shell (iOS / Android) -- simulator/emulator smoke paths verified |
| [`fission-shell-web`](crates/shell/fission-shell-web) | Web shell (WASM + browser) -- checked-in browser smoke path and CLI scaffolding |
| [`fission-cli`](crates/tools/fission-cli) | Project scaffolding CLI and `cargo fission` entrypoint |
| [`fission-diagnostics`](crates/tools/fission-diagnostics) | Structured diagnostic logging and performance tracing |
| [`fission-test`](crates/tools/fission-test) | Test utilities and helpers |
| [`fission-test-driver`](crates/tools/fission-test-driver) | LiveTestClient and JSON test protocol |

## Contributing

Contributions are welcome. Please open an issue or pull request on [GitHub](https://github.com/worka-ai/fission).

## License

MIT -- see [LICENSE](LICENSE).
