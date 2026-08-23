# Interactive canvas example

This graphical example demonstrates both levels of Fission's canvas API:

- `InteractiveViewer` for pan-and-zoom over an arbitrary retained widget tree.
- `InfiniteCanvas` for a declarative node-and-edge editor with selection,
  movement, resizing, snapping, a grid, and edge hit testing.

Run it on a desktop:

```sh
cargo run -p interactive-canvas-example
```

Run the Web target with the Fission CLI:

```sh
fission run --target web --project-dir examples/interactive-canvas
```

The same package declares Android and iOS targets. Static site, SSR, and
Terminal are intentionally absent because these widgets require an interactive
graphical surface.
