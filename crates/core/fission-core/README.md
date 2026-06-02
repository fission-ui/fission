# fission-core

The runtime, widget system, and action/reducer architecture for the Fission UI framework.

`fission-core` is the central crate of the Fission toolkit. It provides the declarative widget tree,
the unidirectional data-flow architecture (actions, reducers, effects), and the runtime that
ties everything together. Applications describe their UI as a `Widget` tree built from composable
widgets, dispatch `Action` values to modify `GlobalState`, and let the runtime lower the tree into
the intermediate representation consumed by platform renderers.

## Architecture overview

```
  From<Component>          Runtime::dispatch()
       |                        |
  ViewHandle<S> + BuildCtxHandle<S>  ActionEnvelope
       |                        |
Widget tree      Reducer(s)
       |                        |
  internal lowering          mutate GlobalState
       |                    + emit Effects
    CoreIR (fission-ir)          |
       |                   EffectEnvelope
  LayoutEngine                  |
       |                 Platform executor
    Renderer
```

1. **Widget layer** -- `From<Component> for Widget` builds the authored tree. Components that need state or action wiring request scoped `ViewHandle<S>` and `BuildCtxHandle<S>` values through `fission::build::current::<S>()`.
2. **Internal lowering** -- Fission converts `Widget` values into `fission-ir` operations
   (paint ops, layout ops, semantics). This is not part of normal application authoring.
3. **Layout** -- `fission-layout` resolves flex, grid, scroll, and absolute positioning.
4. **Rendering** -- Platform backends (Metal, Skia, HTML Canvas) consume the laid-out IR.
5. **Actions** -- User interactions produce `ActionEnvelope` values that the `Runtime`
   dispatches to registered reducers.
6. **Reducers** -- Pure functions that mutate `GlobalState` and optionally emit side-effects
   through `Effects`.
7. **Effects** -- Asynchronous operations (HTTP, file I/O, alerts) that the platform
   executor runs outside the deterministic core.

## Key concepts

### GlobalState

Any `Send + Sync + Debug + 'static` type that implements `GlobalState`. The runtime stores
one instance per concrete type.

### Action / ActionEnvelope

`Action` is a trait for strongly-typed, serialisable event payloads. Actions are transported
as `ActionEnvelope` (type-erased ID + JSON bytes) so the reducer pipeline stays generic.

### BuildCtxHandle

The scoped context handle available during authoring conversion. Use it to:
- **Bind** actions to handlers: `with_reducer!(ctx, MyAction { .. }, my_handler)` or `#[fission_reducer(MyAction)]` for compact one-reducer actions
- **Register portals** (overlays, modals, toasts)
- **Request animations**

### ViewHandle

Read-only scoped access to `GlobalState`, the current theme, i18n registry, layout snapshot, and
animation values.

### Widget

The closed authored widget tree carrier. It has one variant per built-in widget such as `Button`, `Text`, `Row`, and `Column`; application components produce it with `impl From<Component> for Widget`.

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
impl GlobalState for Counter {}

// 2. Define a reducer. This also generates the Increment action.
#[fission_reducer(Increment)]
fn handle_increment(state: &mut Counter) {
    state.count += 1;
}

// 3. Build the widget tree
struct CounterWidget;
impl From<CounterWidget> for Widget {
    fn from(_: CounterWidget) -> Self {
        let (ctx, view) = fission::build::current::<Counter>();
        let on_press = with_reducer!(ctx, Increment, handle_increment);
        Column {
            children: vec![
                Text::new(format!("Count: {}", view.state().count)).into(),
                Button {
                    child: Some(Text::new("Add one").into()),
                    on_press: Some(on_press),
                    ..Default::default()
                }.into(),
            ],
            ..Default::default()
        }.into()
    }
}

```

## Crate feature flags

None at this time -- all functionality is included by default.

## License

MIT
