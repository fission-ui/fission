# fission-render

Display list and rendering abstraction for the Fission UI framework.

## What is this?

`fission-render` sits at the end of the Fission pipeline. After the widget tree
has been compiled to an IR and the layout engine has positioned every node, the
framework flattens the result into a [`DisplayList`] -- an ordered sequence of
[`DisplayOp`] commands that describe exactly what to draw and in what order.

Platform backends implement the [`Renderer`] trait to consume a display list and
produce pixels using whatever GPU or software rasterizer is available (Metal,
Vulkan, wgpu, Skia, etc.).

## Core concepts

| Type | Purpose |
|------|---------|
| [`DisplayList`] | An ordered list of [`DisplayOp`]s plus the bounding rectangle of the scene. |
| [`DisplayOp`] | A single rendering command: draw a rect, draw text, clip, save/restore state, etc. |
| [`Renderer`] | Trait that platform backends implement to turn a display list into pixels. |
| [`Color`] | RGBA color with 8-bit channels. |
| [`TextStyle`] | Font size, color, underline, and optional background highlight. |
| [`TextRun`] | A run of text with a uniform style -- the building block of rich text. |
| [`Fill`] | A solid color fill. |
| [`Stroke`] | A colored stroke with a line width. |
| [`BoxShadow`] | Shadow parameters: color, blur radius, and offset. |
| [`ImageFit`] | How an image scales to fit its layout box (contain, cover, fill, or none). |

## Display operations

The display list uses a save/restore stack model (like HTML Canvas or CoreGraphics):

- **`Save` / `Restore`** -- push and pop the current graphics state.
- **`ClipRect` / `ClipRoundedRect`** -- restrict drawing to a rectangle.
- **`Translate` / `Transform`** -- move or apply a 4x4 matrix to the coordinate space.
- **`DrawRect`** -- fill and/or stroke a rectangle with optional rounded corners and shadow.
- **`DrawText`** -- draw a single-style text string.
- **`DrawRichText`** -- draw multi-style text composed of [`TextRun`]s.
- **`DrawImage`** -- draw an image from a source URI.
- **`DrawPath`** -- draw an SVG-style path string.
- **`DrawSvg`** -- draw inline SVG content.
- **`DrawSurface`** -- blit an external surface (video, embedded web view, etc.).

## Quick example

```rust
use fission_render::*;

let bounds = LayoutRect::new(0.0, 0.0, 800.0, 600.0);
let mut list = DisplayList::new(bounds);

// Draw a blue rectangle with rounded corners
list.push(DisplayOp::DrawRect {
    rect: LayoutRect::new(10.0, 10.0, 200.0, 100.0),
    fill: Some(Fill { color: Color { r: 0, g: 100, b: 255, a: 255 } }),
    stroke: None,
    corner_radius: 8.0,
    shadow: None,
    bounds: LayoutRect::new(10.0, 10.0, 200.0, 100.0),
    node_id: None,
});

// A backend would consume the list like this:
// renderer.render(&list).unwrap();
```

## Implementing a backend

```rust,ignore
use fission_render::{Renderer, DisplayList, DisplayOp};

struct MyGpuRenderer { /* ... */ }

impl Renderer for MyGpuRenderer {
    fn render(&mut self, display_list: &DisplayList) -> anyhow::Result<()> {
        for op in &display_list.ops {
            match op {
                DisplayOp::DrawRect { rect, fill, .. } => { /* draw with your GPU */ }
                DisplayOp::DrawText { text, position, size, color, .. } => { /* shape & rasterize */ }
                // ... handle all variants
                _ => {}
            }
        }
        Ok(())
    }
}
```

## License

Apache-2.0
