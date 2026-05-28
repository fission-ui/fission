# Todo Design System

Todo Design System demonstrates using generated design-system code inside a Fission app. It keeps the app intentionally familiar: a todo list with a draft field, add button, completion checkboxes, clear-completed action, and light/dark theme switch.

Use this example when you want to see how a Design System Package is compiled at build time and then applied through `Env` instead of parsed from JSON at runtime.

## Run it

```bash
cargo run -p todo-design-system
```

## What to look at

- [`build.rs`](build.rs) runs the design-system code generator before the app compiles.
- [`src/main.rs`](src/main.rs) includes the generated Rust file from `OUT_DIR` and applies the generated theme in `with_sync_env`.
- `TodoState` stores app data plus the selected `DesignMode`.
- `TodoApp::build` shows component sizes, button variants, checkboxes, cards, text input, and generated tokens used together.
- [`Cargo.toml`](Cargo.toml) shows the `fission-design-system-codegen` build dependency.

## Features exercised

- Build-time design-system generation.
- Generated theme application through `env.theme`.
- Component styles for buttons, cards, text inputs, and checkboxes.
- Light/dark mode switching with ordinary reducers.
- A compact real app structure suitable for design-system experiments.

## Learning path

Start with [`build.rs`](build.rs), then open [`src/main.rs`](src/main.rs) and find `include!(concat!(env!("OUT_DIR"), ...))`. That is the handoff from generated design-system Rust into normal app code. The `with_sync_env` callback is where the current app state chooses which generated theme to expose to widgets.
