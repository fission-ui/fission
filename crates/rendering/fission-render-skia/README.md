# fission-render-skia

`fission-render-skia` is Fission's safe renderer adapter for the framework-owned
Skia ABI. It translates backend-neutral `InteractiveFrame` submissions into
batched Skia work without exposing Skia objects to applications, widgets,
layout, or platform hosts.

The renderer adapter now includes deterministic headless Skia raster and a
native Ganesh presentation profile for Linux Vulkan (Wayland, Xlib, and XCB),
macOS Metal (AppKit), and iOS Metal (UIKit).
The Ganesh profile becomes constructible only with a matching native artifact.
Both paths provide explicit lifecycle, recovery, memory-pressure, diagnostic,
thread-affinity, and teardown behavior together with save/restore, rectangular
and rounded clipping, finite 2D affine transforms, complete rectangle and SVG
path fills/strokes, gradients, dash/cap/join state, and outer or inset box
shadows. Backdrop blur is lowered as an atomic native filter in physical
coordinates, including rounded bounds and device-scaled blur sigma. A paired
`SkiaRasterProfile` retains SkParagraph paint data from the authoritative layout
result. In-memory images are resolved only from each submitted frame's resource
snapshot, decoded through SkCodec under a bounded driver-owned cache, and
painted with all current fit, alignment, clipping, and device-scale semantics.
Document-paint SVG is retained in bounded, driver-owned SkSVGDOM cache entries,
while Fission fill/stroke overrides retain the existing path,
rectangle, polygon, viewBox, gradient, dash, cap, and join semantics through
the backend-neutral paint commands. Cache hints on display lists and render
layers record immutable Skia pictures in a bounded driver-owned cache. Hits
require exact retained content, physical scale, current frame resource entries,
and authoritative paragraph geometry/draw-data identities; destination-dependent
content falls back to ordinary recursive lowering. The defaults can be tuned
with `FISSION_SKIA_PICTURE_CACHE_BYTES` and
`FISSION_SKIA_PICTURE_CACHE_ENTRIES`. `SkiaGaneshProfile` uses those same
compiler, image, SVG, picture, and authoritative SkParagraph resources while
rendering directly into a Ganesh swapchain surface; it never routes pixels
through wgpu or a raster readback/upload path. Its platform host must keep the
raw native display and window handles live until detach. Its sole Ganesh GPU
resource cache is capped at 64 MiB by default, can be set in bytes with
`FISSION_SKIA_GPU_CACHE_BYTES`, reports current entries and bytes through
backend diagnostics, and purges unlocked resources on host memory pressure.
The environment setting is read once when the driver is created and the frozen
limit is reapplied when device recovery creates a new context. External-surface/
3D interop, other image sources, other filters, and CanvasKit remain behind the
same Fission contracts while their production implementations are completed.

This crate deliberately reports only semantics that its current adapter can
honor. The initial foundation profile is not advertised as a complete
production graphical profile.

The default `skia-prebuilt` feature consumes Fission's verified native artifact.
`skia-build-from-source` is the explicit source-build path, while `test-shim`
exists only for ABI and ownership tests and is never a renderer profile.
Creating a Ganesh driver or session fails clearly when the selected artifact
does not advertise Ganesh, native presentation, and the target platform's
Vulkan or Metal feature bits.
