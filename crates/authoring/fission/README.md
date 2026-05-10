# Fission

Cross-platform, GPU-accelerated UI framework for Rust. Fission uses a Flutter-inspired widget architecture with a deterministic state management model built on serializable actions and reducers. Rendering is powered by [Vello](https://github.com/linebender/vello) and [wgpu](https://wgpu.rs/), delivering high-performance 2D graphics on every platform. Desktop (macOS, Linux, Windows) is fully supported today; iOS, Android, and Web (WASM) targets are in progress.

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

## Deterministic state management

State changes flow exclusively through **Actions** and **Reducers**. Actions are strongly-typed, serializable structs. Reducers are pure functions that take `(&mut State, Action, &mut ReducerContext)` and mutate state. No shared mutable state, no callbacks, no implicit side effects.

This makes the UI fully deterministic: given the same state and the same sequence of actions, you get the same output -- guaranteed.

```rust
#[derive(Action, Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
struct Increment;

// Bind an action to a reducer inline
let action = ctx.bind(Increment, |s: &mut CounterState, _, _| {
    s.count += 1;
});

// Or define a named handler
fn on_increment(state: &mut CounterState, _action: Increment, _ctx: &mut ReducerContext<CounterState>) {
    state.value += 1;
}
let action = ctx.bind(Increment, on_increment as Handler<CounterState, Increment>);
```

## Effects system

Reducers must be pure -- they cannot perform I/O. When async work is needed, reducers emit **Effects** through the `ReducerContext`. The shell-owned async host fulfills typed capability operations, jobs, and services outside the deterministic core and dispatches the result back as a bound callback action.

Use capability operations for host/device access and typed jobs for app-defined async work.

```rust
fn pick_file(state: &mut MyState, _action: PickFile, ctx: &mut ReducerContext<MyState>) {
    ctx.effects
        .capability(PICK_OPEN_FILES, PickOpenFilesRequest::default())
        .on_ok(ctx.effects.bind(FilePicked, handle_picked as fn(&mut MyState, FilePicked, _)))
        .on_err(ctx.effects.bind(FilePickFailed, handle_pick_failed as fn(&mut MyState, FilePickFailed, _)));
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

- **Colors** -- semantic palette (primary, secondary, surface, background, error, border, text) with `on_*` counterparts
- **Spacing** -- consistent spacing scale (xs through xxxl)
- **Typography** -- font families, sizes, weights, and line heights for display, headline, title, body, label, and caption
- **Corner radii** -- none, xs, sm, md, lg, xl, full
- **Elevations** -- box shadow presets for depth levels 0-5

Themes also include per-component overrides (button colors, input borders, card shadows, etc.).

```rust
// Use the default light theme
let light = Theme::default();

// Switch to dark mode
let dark = Theme::dark();

// Access tokens
let primary = view.env.theme.tokens.colors.primary;
let padding = view.env.theme.tokens.spacing.md;
```

## Internationalisation

The `fission-i18n` crate provides a registry-based translation system:

```rust
use fission::i18n::{I18nRegistry, TranslationBundle, Locale};

let mut registry = I18nRegistry::new();
registry.add_bundle(TranslationBundle {
    locale: Locale::from("en-US"),
    messages: [("greeting".into(), "Hello!".into())].into(),
});
let msg = registry.get(&Locale::from("en-US"), "greeting");
```

In widgets, use `TextContent::Key("greeting")` to look up translated strings at render time. The active locale is stored in the environment and flows through the widget tree automatically.

## Accessibility and semantics

Every built-in widget emits **Semantics** metadata: role, label, value, and focusable state. The semantic tree is extracted after layout and exposed to platform accessibility APIs.

Supported roles include `Button`, `Text`, `TextInput`, `Image`, `Checkbox`, `Switch`, `Dialog`, `Slider`, `Input`, `List`, and more.

Focus traversal is managed by the runtime -- Tab/Shift-Tab cycles through focusable nodes in tree order. Screen reader support is on the roadmap via platform accessibility bridges (NSAccessibility on macOS, AT-SPI on Linux, UIA on Windows).

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

Custom animation properties are supported via `AnimationPropertyId::custom("my_property")`. Set `repeat: true` for looping animations (spinners, pulsing indicators). Easing curves are built into the runtime.

## Icons

Material Design icons are bundled in the `fission-icons` crate, generated at build time from SVG data. No external assets or checkout steps required.

```rust
use fission::icons::material;

Icon::svg(material::action::search::regular()).size(24.0)
```

Icons are organized by Material Design categories (action, alert, av, communication, content, editor, file, hardware, image, maps, navigation, notification, social, toggle).

## Custom render objects

For widgets that need direct control over the rendering pipeline -- code editors, charts, visualizations, games -- implement the `LowerDyn` trait:

```rust
impl LowerDyn for MyCustomRenderer {
    fn lower_dyn(&self, cx: &mut LoweringContext) -> NodeId {
        // Emit layout and paint operations directly into the IR
        let paint = NodeBuilder::new(cx.next_node_id(), Op::Paint(PaintOp::DrawRect {
            fill: Some(Fill { color: IrColor::RED }),
            stroke: None,
            corner_radius: 4.0,
            shadow: None,
        })).build(cx);

        let mut layout = NodeBuilder::new(cx.next_node_id(), Op::Layout(LayoutOp::Box {
            width: Some(200.0), height: Some(100.0),
            ..Default::default()
        }));
        layout.add_child(paint);
        layout.build(cx)
    }
}

// Use it in a widget
Node::Custom(CustomNode {
    debug_tag: "MyRenderer".into(),
    lowerer: Some(Arc::new(MyCustomRenderer { /* ... */ })),
})
```

The Fission Editor example application uses this mechanism for its entire code editing surface.

## Testing

Fission provides two testing approaches:

- **`TestDriver`** -- headless widget testing. Build a widget tree, apply actions, and assert on the resulting node structure without a GPU.
- **`LiveTestClient`** -- full integration testing. Connects to a running application over HTTP (via `FISSION_TEST_CONTROL_PORT`) and sends commands: `Tap`, `TapText`, `TypeText`, `PressKey`, `Scroll`, `Screenshot`, `GetText`, `GetTree`.

GPU screenshot capture enables visual regression testing. Because the framework is deterministic, same state + same actions = same rendered output, guaranteed.

```rust
use fission::test_driver::{LiveTestClient, TestCommand};

let client = LiveTestClient::connect(9876).await?;
client.send(TestCommand::TapText { text: "Increment".into() }).await?;
let texts = client.send(TestCommand::GetText {}).await?;
```

## Platform support

| Platform | Status |
|----------|--------|
| macOS | Supported |
| Linux | Supported |
| Windows | Supported |
| iOS | In progress |
| Android | In progress |
| Web (WASM) | In progress |

## Diagnostics

Structured diagnostic system covering every stage of the frame lifecycle. Events are categorized (Frame, Diff, Layout, Paint, Raster, Input, Semantics, Animation, Media, Invariants, Test) with configurable severity levels (Error, Warn, Info, Debug, Trace).

Configure via environment variables or programmatically:

```rust
use fission::diagnostics::prelude::*;

init_from_env(); // reads FISSION_DIAG_LEVEL, FISSION_DIAG_CATEGORIES, FISSION_DIAG_FILE
```

Output is structured JSON, suitable for piping into analysis tools or dashboards.

## Crate map

| Crate | Description |
|---|---|
| [`fission`](https://crates.io/crates/fission) | Facade crate -- single dependency for applications |
| `fission-core` | Runtime, built-in widgets, actions, reducers, effects, animations |
| `fission-ir` | Intermediate representation -- the flat node graph between widgets and layout |
| `fission-layout` | Constraint-based layout engine (flexbox + box model + grid) |
| `fission-theme` | Design tokens, component themes, dark/light mode |
| `fission-i18n` | Internationalisation -- locale registry and string lookups |
| `fission-semantics` | Accessibility roles and semantic tree types |
| `fission-widgets` | Higher-level authoring widgets (Modal, Popover, Tabs, SplitView, etc.) |
| `fission-macros` | Derive macros (`#[derive(Action)]`) |
| `fission-icons` | Material Design icon set, generated from bundled SVGs |
| `fission-render` | Rendering primitives -- display list, paint ops, text styles |
| `fission-render-vello` | Vello/wgpu rendering backend |
| `fission-shell` | Shared shell abstractions (event loop, windowing) |
| `fission-shell-desktop` | Desktop shell -- winit + Vello + wgpu integration |
| `fission-shell-mobile` | Mobile shell (iOS / Android) -- in progress |
| `fission-shell-web` | Web shell (WASM + WebGPU) -- in progress |
| `fission-diagnostics` | Structured diagnostic logging and performance tracing |
| `fission-test` | Test utilities and helpers |
| `fission-test-driver` | LiveTestClient and JSON test protocol |

## License

MIT -- see [LICENSE](https://github.com/worka-ai/fission/blob/main/LICENSE).
