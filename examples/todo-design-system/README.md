# Todo Design System

Todo Design System is the reference for styling a Fission app with a generated design system. It keeps the app familiar: a task input, a list of tasks to check off, a clear-completed action and a light/dark switch. Every colour, space, type size and component style comes from the generated theme, and every label comes from a translation file.

## Run it

```bash
cargo run -p todo-design-system
```

## What to look at

- [`build.rs`](build.rs) runs the design-system code generator before the app compiles.
- [`src/lib.rs`](src/lib.rs) includes the generated `TodoDesignSystem` from `OUT_DIR`, installs the translations, and applies the generated theme in `with_sync_env`.
- [`src/state.rs`](src/state.rs) holds the tasks and the selected `DesignMode`, with a named reducer for each action.
- [`src/widgets/`](src/widgets) has one widget per file. Each reads `view.env().theme.tokens` and `view.tr(...)` rather than naming colours or text.
- [`i18n/`](i18n) holds the English and Spanish labels.

## Features exercised

- Build-time design-system generation.
- Generated theme application through `env.theme`.
- Component styles for buttons, cards, text input, checkboxes, badges and segmented controls.
- Responsive layout: the header and task input stack on narrow widths.
- Translations embedded with `include_str!`.
- Named reducers bound with `ctx.bind`.

## Learning path

Start with [`build.rs`](build.rs), then open [`src/lib.rs`](src/lib.rs) and find `include!(concat!(env!("OUT_DIR"), ...))`. That is the handoff from generated design-system Rust into normal app code. The `with_sync_env` callback is where the current app state chooses which generated theme to expose to widgets. Then read the widgets from [`todo_app.rs`](src/widgets/todo_app.rs) down.
