# fission-render-software

`fission-render-software` is Fission's retained reference and conformance CPU
renderer for `RenderScene` and `DisplayList` content. It rasterizes with
tiny-skia, uses fontdue for glyph rendering, and returns a complete
premultiplied RGBA pixel buffer without requiring a window, GPU, Vello, wgpu,
Winit, or Parley.

Production graphical shells do not select this crate. Native software
rendering uses `fission-render-skia`'s Skia raster profile and interactive Web
software rendering uses CanvasKit's Skia raster surface. This crate remains in
the workspace for focused comparison and conformance coverage while that is
useful.

The crate also owns the software image cache and lazy packaged-font registry.
Shells can observe cache generation, pending work, and statistics to schedule a
new frame when asynchronous image decoding completes.

```rust
use fission_render::{Color, RenderScene};
use fission_render_software::SoftwareRenderer;

# fn render(scene: &RenderScene) -> anyhow::Result<Vec<u8>> {
SoftwareRenderer::render(
    scene,
    800,
    600,
    Color {
        r: 255,
        g: 255,
        b: 255,
        a: 255,
    },
    1.0,
)
# }
```

This crate deliberately does not provide a graphics-session driver: it owns
rasterization and pixel output, but not window attachment, presentation, or
surface/device recovery. Keeping it outside production selection prevents it
from becoming an implicit fallback beneath a Skia profile.
