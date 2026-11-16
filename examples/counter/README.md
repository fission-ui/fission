# Counter

Counter is the smallest useful Fission application in the repository. It demonstrates the recommended Compose authoring model: define a component, keep small retained UI state in its `state` section, and place small state transitions beside the buttons that own them.

Use this example first if you are new to Fission. It intentionally avoids extra styling and platform configuration so the state/action/widget flow is easy to see.

## Run it

```bash
cargo run -p counter
```

## What to look at

- [`src/lib.rs`](src/lib.rs) contains the complete shared component so the core pattern is visible without jumping between modules.
- `CounterApp` uses `fission_component!` and a `state` section for retained widget-local state.
- The two inline `reduce` handlers show where one-component state transitions belong.
- Literal reducer keys and semantic identifiers give actions stable identity and make the controls accessible to live tests.
- The tree uses imported `Text`, `Button`, `Row`, `Column`, and `Container` types directly; Compose has no separate widget catalogue.

## Features exercised

- Desktop app startup through `DesktopApp::new(...).run()`.
- Compose components, retained local state, and inline reducers.
- Button press handling through the same typed action envelopes used by reusable reducers.
- Rebuilding UI from retained local widget state after actions run.
- Active design-system tokens for spacing, colour, and typography.

## Learning path

Read the component from top to bottom, then look at the thin shell entrypoints. When you build larger examples, app-wide domain data moves into `GlobalState`, reusable domain transitions become named reducers, and temporary UI memory stays local to the component that owns it. Continue with the [`compose` example](../compose/) to see that structure across multiple routes and components.
