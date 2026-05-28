# Editor

Editor is a larger desktop example that exercises Fission as an application framework rather than a widget sampler. It models the structure of a code editor with a file tree, tab bar, command palette, diagnostics, search, context menus, completion popups, terminal panel, syntax support, and editor-specific render nodes.

Use this example when you want to study how to organize a substantial Fission app into focused modules and how more advanced UI surfaces can cooperate around a shared application model.

## Run it

```bash
cargo run -p fission-editor
```

## What to look at

- [`src/main.rs`](src/main.rs) wires the desktop shell and top-level editor UI.
- [`src/model.rs`](src/model.rs) defines the shared editor state used by the feature panels.
- [`src/editor_surface.rs`](src/editor_surface.rs) and [`src/editor_render_node.rs`](src/editor_render_node.rs) show the main editing surface and custom render-node integration.
- [`src/file_tree.rs`](src/file_tree.rs), [`src/tab_bar.rs`](src/tab_bar.rs), [`src/status_bar.rs`](src/status_bar.rs), and [`src/menu_bar.rs`](src/menu_bar.rs) show persistent application chrome.
- [`src/command_palette.rs`](src/command_palette.rs), [`src/context_menu.rs`](src/context_menu.rs), [`src/completion_popup.rs`](src/completion_popup.rs), and [`src/hover_tooltip.rs`](src/hover_tooltip.rs) show overlay-style UI.
- [`src/lsp/`](src/lsp) and [`src/plugin/`](src/plugin) show how protocol and plugin concerns are separated from widget rendering.

## Features exercised

- Production-scale module organization for a desktop app.
- Text editing surfaces, diagnostics, search, menus, popups, and terminal embedding.
- `terminal-widget` feature use through [`Cargo.toml`](Cargo.toml).
- Test support through `fission-test` and `fission-test-driver` dev dependencies.

## Learning path

Begin with [`src/model.rs`](src/model.rs), then trace one feature at a time. For example, follow command palette state into [`src/command_palette.rs`](src/command_palette.rs), or follow diagnostics state into [`src/diagnostics_panel.rs`](src/diagnostics_panel.rs). The example is intentionally split so each feature can be studied without reading the whole app at once.
