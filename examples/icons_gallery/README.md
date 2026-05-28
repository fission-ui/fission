# Icons Gallery

Icons Gallery lists the Material icon set exposed through the Fission facade. It is intentionally simple: the app builds a large set of icon rows once, stores them behind `Arc`, and renders them through `LazyColumn` so the gallery remains usable even with many icon variants.

Use this example when you want to browse icon names or learn how to render many similarly sized rows efficiently.

## Run it

```bash
cargo run -p icons_gallery
```

## What to look at

- [`src/main.rs`](src/main.rs) contains the full app.
- `build_icon_rows` shows how to iterate over `fission::icons::material::all_icons()` and convert icon functions into `Icon::svg(...)` nodes.
- `ICON_ROWS` shows how static example data can be prepared once with `lazy_static!`.
- `IconsApp::build` shows `LazyColumn` with a fixed `item_height`.

## Features exercised

- Material icon reflection through the `icons-reflection` feature in [`Cargo.toml`](Cargo.toml).
- `Icon::svg(...)` rendering.
- `LazyColumn` for large fixed-height lists.
- Simple desktop app startup through `DesktopApp`.

## Learning path

Start with `build_icon_rows`. If you want to use icons in your own app, the key line is `Icon::svg(func()).size(24.0).into_node()`. The rest of the example is list layout and styling.
