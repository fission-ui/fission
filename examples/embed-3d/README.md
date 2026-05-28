# 3D Embed

3D Embed demonstrates placing a bounded 3D scene inside a normal Fission widget layout. The important part is not the cube itself; it is the way a specialized scene widget participates in ordinary layout alongside text, containers, borders, and spacing.

Use this example when you want to learn how host-backed or renderer-backed surfaces are embedded without taking over the whole application window.

## Run it

```bash
cargo run -p embed-3d
```

## What to look at

- [`src/lib.rs`](src/lib.rs) defines `Scene3DEmbedApp`, the app state, and the `Scene3D` widget tree.
- [`src/main.rs`](src/main.rs) is the desktop entrypoint that runs the library app.
- [`Cargo.toml`](Cargo.toml) enables the `three-d` feature on the Fission facade crate.

## Features exercised

- `Scene3D`, `Primitive3D`, and `Point3D` from `fission::three_d`.
- Embedding a fixed-size specialized surface in a larger `Column`.
- Theming through `view.env.theme.tokens.colors`.
- Keeping the reusable app in `lib.rs` so tests and entrypoints can share it.

## Learning path

Open [`src/lib.rs`](src/lib.rs) and find `Scene3D::new()`. The scene is built like any other widget and then wrapped in a `Container` that gives it explicit width, height, and border. That is the pattern to copy for other bounded embedded surfaces.
