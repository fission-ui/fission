# fission-render

Backend-neutral display lists, retained render scenes, and interactive graphics
contracts for the Fission UI framework.

Fission lowers widgets and layout into a `RenderScene`. Its ordered `DisplayOp`
values describe paint and composition semantics without exposing Vello, wgpu,
Skia, Winit, or another implementation type. Static-site, SSR, and terminal
targets can share these neutral types without linking an interactive renderer.

## Core types

| Type | Purpose |
|---|---|
| `DisplayList` | Ordered paint operations and their logical bounds. |
| `DisplayOp` | Shapes, text, images, paths, SVG, clips, layers, and external surfaces. |
| `RenderScene` | Retained tree of paint lists and compositing layers. |
| `Renderer` | Small scene-consumer contract for headless and compatibility renderers. |
| `InteractiveFrame` | A scene bound to frame metadata, resources, and external producers. |
| `GraphicsBackendSession` | Capability, lifecycle, validation, render, present, recovery, and readback boundary. |

The interactive contracts are hidden from generated documentation while the
current backends exercise them. They are public at the Rust crate boundary so
separately packaged backend crates can implement the contract, but they are not
yet a stable application-authoring API.

## Display-list example

```rust
use fission_render::{Color, DisplayList, DisplayOp, Fill, LayoutRect};

let bounds = LayoutRect::new(0.0, 0.0, 800.0, 600.0);
let mut list = DisplayList::new(bounds);
list.push(DisplayOp::DrawRect {
    rect: LayoutRect::new(10.0, 10.0, 200.0, 100.0),
    fill: Some(Fill::Solid(Color {
        r: 0,
        g: 100,
        b: 255,
        a: 255,
    })),
    stroke: None,
    corner_radius: 8.0,
    shadow: None,
    bounds: LayoutRect::new(10.0, 10.0, 200.0, 100.0),
    node_id: None,
});
```

## Implementing a scene consumer

```rust
use fission_render::{RenderScene, Renderer};

struct Inspector;

impl Renderer for Inspector {
    fn render_scene(&mut self, scene: &RenderScene) -> anyhow::Result<()> {
        for operation in scene.flatten().ops {
            println!("{operation:?}");
        }
        Ok(())
    }
}
```

The lifecycle and capability contracts in `fission_render::backend` are the
target boundary for interactive backends. `GraphicsBackendSession` provides a
non-bypassable validation and lifecycle gate for adapters implemented against
that contract. Existing production hosts are being migrated incrementally and
still use compatibility integration paths where no session driver exists yet.

## License

MIT
