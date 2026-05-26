# fission-render-vello

Vello rendering backend for the Fission UI framework.

`fission-render-vello` is the primary GPU rendering backend for Fission. It implements the
`Renderer` trait by translating `DisplayList` operations into a Vello `Scene` graph, which is
then rasterized on the GPU via `wgpu`. Text shaping and layout are handled by
[Parley](https://github.com/linebender/parley) through the `VelloTextMeasurer`, which also
implements the `TextMeasurer` trait for the layout engine. The crate includes caches for text
layouts (simple and rich), decoded images, and parsed SVG paths to keep frame times low.

## Key types

| Type | Description |
|------|-------------|
| `VelloRenderer<'a>` | Implements `fission_render::Renderer`. Writes display ops into a Vello `Scene` using the current affine transform stack. Handles rectangles, text, rich text (with per-run styling and background highlights), images, SVG paths, inline SVG, and box shadows. |
| `VelloTextMeasurer` | Implements `fission_layout::TextMeasurer`. Wraps a Parley `FontContext` and `LayoutContext` with an LRU-style layout cache (4096 simple entries, 2048 rich entries). Provides `measure`, `measure_rich_text`, `hit_test`, `get_line_metrics`, and `get_caret_position`. |
| `ParleyBrush` | A simple `[u8; 4]` RGBA brush type used as the generic parameter for `parley::layout::Layout`. |

## Supported display ops

- `DrawRect` with fill, stroke, rounded corners, and box shadow
- `DrawText` with font size, color, underline, and optional caret rendering
- `DrawRichText` with per-run font size, color, underline, and background highlight
- `DrawImage` with Fill, Contain, Cover, and None fit modes (image cache)
- `DrawPath` from SVG path data
- `DrawSvg` with viewBox-aware scaling and fill/stroke (SVG parse cache)
- `DrawSurface` (video surface placeholder)
- `Save` / `Restore` / `ClipRect` / `ClipRoundedRect` / `Translate` / `Transform`

## Usage example

```rust
use fission_render_vello::{VelloRenderer, VelloTextMeasurer};
use parley::FontContext;
use std::sync::{Arc, Mutex};
use vello::Scene;

// Set up text measurement
let font_cx = Arc::new(Mutex::new(FontContext::new()));
let measurer = Arc::new(VelloTextMeasurer::new(font_cx));

// Render a frame
let mut scene = Scene::new();
let mut renderer = VelloRenderer::new(&mut scene, measurer, 2.0 /* scale factor */);
renderer.render(&display_list)?;
// `scene` is now ready to be submitted to a Vello `Renderer` backed by wgpu.
```

## Status

**Supported** -- this is the default and recommended rendering backend for Fission on desktop.
Used in production by `fission-shell-desktop`.

## License

MIT -- see the [Fission repository](https://github.com/worka-ai/fission) for full documentation at https://fission.rs.
