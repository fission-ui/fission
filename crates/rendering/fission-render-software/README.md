# fission-render-software

`fission-render-software` is Fission's platform-independent CPU renderer for
retained `RenderScene` and `DisplayList` content. It rasterizes with tiny-skia,
uses fontdue for glyph rendering, and returns a complete premultiplied RGBA
pixel buffer without requiring a window, GPU, Vello, wgpu, Winit, or Parley.

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
surface/device recovery. A host shell composes those lifecycle responsibilities
through Fission's session boundary.
