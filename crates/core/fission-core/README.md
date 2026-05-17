# fission-core

The runtime, widget system, and action/reducer architecture for the Fission UI framework.

`fission-core` is the central crate of the Fission toolkit. It provides the declarative widget tree,
the unidirectional data-flow architecture (actions, reducers, effects), and the runtime that
ties everything together. Applications describe their UI as a `Node` tree built from composable
widgets, dispatch `Action` values to modify `AppState`, and let the runtime lower the tree into
the intermediate representation consumed by platform renderers.

## Architecture overview

```
  Widget::build()          Runtime::dispatch()
       |                        |
  View<S> + BuildCtx<S>     ActionEnvelope
       |                        |
    Node tree                Reducer(s)
       |                        |
  LoweringContext            mutate AppState
       |                    + emit Effects
    CoreIR (fission-ir)          |
       |                   EffectEnvelope
  LayoutEngine                  |
       |                 Platform executor
    Renderer
```

1. **Widget layer** -- `Widget::build()` receives read-only state through `View<S>` and
   a mutable `BuildCtx<S>` for binding actions. It returns a `Node` tree.
2. **Lowering** -- Each `Node` variant implements the `Lower` trait, converting itself into
   `fission-ir` operations (paint ops, layout ops, semantics).
3. **Layout** -- `fission-layout` resolves flex, grid, scroll, and absolute positioning.
4. **Rendering** -- Platform backends (Metal, Skia, HTML Canvas) consume the laid-out IR.
5. **Actions** -- User interactions produce `ActionEnvelope` values that the `Runtime`
   dispatches to registered reducers.
6. **Reducers** -- Pure functions that mutate `AppState` and optionally emit side-effects
   through `Effects`.
7. **Effects** -- Asynchronous operations (HTTP, file I/O, alerts) that the platform
   executor runs outside the deterministic core.

## Key concepts

### AppState

Any `Send + Sync + Debug + 'static` type that implements `AppState`. The runtime stores
one instance per concrete type.

### Action / ActionEnvelope

`Action` is a trait for strongly-typed, serialisable event payloads. Actions are transported
as `ActionEnvelope` (type-erased ID + JSON bytes) so the reducer pipeline stays generic.

### BuildCtx

The mutable context passed to `Widget::build()`. Use it to:
- **Bind** actions to handlers: `with_reducer!(ctx, MyAction { .. }, my_handler)` or `#[fission_reducer(MyAction)]` for compact one-reducer actions
- **Register portals** (overlays, modals, toasts)
- **Request animations**

### View

Read-only access to `AppState`, the current theme, i18n registry, layout snapshot, and
animation values.

### Node

A serialisable enum with one variant per built-in widget (Button, Text, Row, Column, etc.)
plus a `Custom` escape hatch.

### Effects

Reducers that need async work (network, disk, timers) push `EffectEnvelope` values through
`Effects`. The platform executor fulfils them and dispatches the `on_ok` / `on_err` callbacks
back into the action pipeline.

## Quick example

```rust
use fission::prelude::*;

// 1. Define state
#[derive(Debug, Default)]
struct Counter { count: i32 }
impl AppState for Counter {}

// 2. Define a reducer. This also generates the Increment action.
#[fission_reducer(Increment)]
fn handle_increment(state: &mut Counter) {
    state.count += 1;
}

// 3. Build the widget tree
struct CounterWidget;
impl Widget<Counter> for CounterWidget {
    fn build(&self, ctx: &mut BuildCtx<Counter>, view: &View<Counter>) -> Node {
        let on_press = with_reducer!(ctx, Increment, handle_increment);
        Column {
            children: vec![
                Text::new(format!("Count: {}", view.state.count)).into_node().into(),
                Button {
                    child: Some(Box::new(Text::new("Add one").into_node())),
                    on_press: Some(on_press),
                    ..Default::default()
                }.into_node().into(),
            ],
            ..Default::default()
        }.into_node()
    }
}

```

## Crate feature flags

None at this time -- all functionality is included by default.

## License

MIT
