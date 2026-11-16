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

The root application uses Compose to assemble controls and the canvas, while
the domain state, durable node identifiers and interaction actions remain
ordinary Rust. Nodes are keyed by their durable `CanvasNodeId`; list positions
are never used as retained identity. The example also uses the built-in canvas
selection, viewport and snapping contracts instead of recreating pointer and
coordinate handling in application code.

Use `InteractiveViewer` when an application needs pan and zoom around one
retained child. Use `InfiniteCanvas` when the product is a node-and-edge editor
and needs selection, movement, resizing, connection routing and overlays as one
coherent widget. The example keeps both paths visible so developers can choose
the smallest abstraction that owns the required behaviour.

See the [widget catalogue](https://fission.rs/reference/widgets/catalog/) for
the complete canvas types and feature gates.

## Licence

Apache-2.0
