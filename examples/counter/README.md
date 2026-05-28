# Counter

Counter is the smallest useful Fission application in the repository. It demonstrates the basic mental model: define app state, define reducers that update that state, build a widget tree from the current state, and bind buttons to actions.

Use this example first if you are new to Fission. It intentionally avoids extra styling and platform configuration so the state/action/widget flow is easy to see.

## Run it

```bash
cargo run -p counter
```

## What to look at

- [`src/main.rs`](src/main.rs) contains the whole app in one file so the core pattern is visible without jumping between modules.
- `CounterState` shows the app state type and the `AppState` implementation.
- `increment` and `decrement` show `#[fission_reducer]` on simple state mutations.
- `CounterApp::build` shows `with_reducer!`, `Text`, `Button`, `Row`, `Column`, and `Container` in a compact widget tree.

## Features exercised

- Desktop app startup through `DesktopApp::new(...).run()`.
- Fission reducer macros for action generation.
- Button press handling through bound action envelopes.
- Rebuilding UI from state after actions run.

## Learning path

Read the file from top to bottom. The order mirrors a typical beginner app: state first, reducers second, widgets third, shell startup last. When you build larger examples, the same pieces usually move into separate modules but keep the same responsibilities.
