# Counter

Counter is the smallest useful Fission application in the repository. It demonstrates the v2 authoring model: define a component, keep small retained UI state on that component with `#[local_state]`, bind buttons to generated actions, and convert the component into a `Widget` tree.

Use this example first if you are new to Fission. It intentionally avoids extra styling and platform configuration so the state/action/widget flow is easy to see.

## Run it

```bash
cargo run -p counter
```

## What to look at

- [`src/main.rs`](src/main.rs) contains the whole app in one file so the core pattern is visible without jumping between modules.
- `CounterApp` shows `#[fission_component]` and `#[local_state]` for retained widget-local state.
- `increment` and `decrement` show `#[fission_reducer]` on simple local state mutations.
- `impl From<CounterApp> for Widget` is the public component boundary.
- The widget tree shows `widgets!`, `Text`, `Button`, `Row`, `Column`, and `Container` without app-wide state.

## Features exercised

- Desktop app startup through `DesktopApp::new(...).run()`.
- Fission component, local state, and reducer macros.
- Button press handling through bound action envelopes.
- Rebuilding UI from retained local widget state after actions run.

## Learning path

Read the file from top to bottom. The order mirrors a typical beginner app: component first, reducers second, widget tree third, shell startup last. When you build larger examples, app-wide domain data moves into `GlobalState`, but small UI-only state can stay local to the component that owns it.
