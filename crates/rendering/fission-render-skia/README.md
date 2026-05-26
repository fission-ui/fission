# fission-render-skia

Skia rendering backend for the Fission UI framework.

`fission-render-skia` implements the `Renderer` and `TextMeasurer` traits using the
[Skia](https://skia.org/) 2D graphics library (via `skia-safe`). It translates the Fission
`DisplayList` into Skia canvas calls -- rectangles, rounded rects, text paragraphs, images, and
box shadows -- and provides `SkiaTextMeasurer` for accurate text measurement, hit testing, line
metrics, and caret positioning backed by Skia's paragraph layout engine.

## Key types

| Type | Description |
|------|-------------|
| `SkiaRenderer<'a>` | Implements `fission_render::Renderer`. Holds a reference to a Skia `Canvas` and renders `DisplayList` ops (fill, stroke, shadow, text, rich text, images). |
| `SkiaTextMeasurer` | Implements `fission_layout::TextMeasurer`. Uses Skia paragraph layout for `measure`, `hit_test`, `get_line_metrics`, and `get_caret_position`. |

## Usage example

```rust
use fission_render_skia::{SkiaRenderer, SkiaTextMeasurer};

// Given a Skia canvas from your windowing backend:
let mut renderer = SkiaRenderer::new(&canvas);
renderer.render(&display_list)?;

// Text measurement for the layout engine:
let measurer = SkiaTextMeasurer;
let (width, height) = measurer.measure("Hello", 16.0, Some(200.0));
```

## Supported display ops

- `DrawRect` with fill, stroke, corner radius, and box shadow
- `DrawText` and `DrawRichText` via Skia paragraph builder
- `DrawImage` with Fill, Contain, Cover, and None fit modes
- `DrawSurface` (placeholder rendering)
- `Save` / `Restore` / `ClipRect` / `ClipRoundedRect` / `Translate` / `Transform`
- `DrawPath` and `DrawSvg` are recognized but not yet implemented (log a warning)

## Status

**Experimental** -- functional but not the primary rendering path. The default backend is
`fission-render-vello`. Use this backend when Skia is preferred or when Vello/wgpu are
unavailable.

## License

MIT -- see the [Fission repository](https://github.com/worka-ai/fission) for full documentation.
