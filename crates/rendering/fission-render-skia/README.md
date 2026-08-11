# fission-render-skia

`fission-render-skia` is Fission's safe renderer adapter for the framework-owned
Skia ABI. It translates backend-neutral `InteractiveFrame` submissions into
batched Skia work without exposing Skia objects to applications, widgets,
layout, or platform hosts.

The first implementation profile is deterministic headless Skia raster. It
provides explicit lifecycle, readback, recovery, memory-pressure, diagnostic,
thread-affinity, and teardown behavior together with save/restore, rectangular
and rounded clipping, finite 2D affine transforms, complete rectangle and SVG
path fills/strokes, gradients, dash/cap/join state, and outer or inset box
shadows. Backdrop blur is lowered as an atomic native filter in physical
coordinates, including rounded bounds and device-scaled blur sigma. A paired
`SkiaRasterProfile` retains SkParagraph paint data from the authoritative layout
result. In-memory images are resolved only from each submitted frame's resource
snapshot, decoded through SkCodec under a bounded driver-owned cache, and
painted with all current fit, alignment, clipping, and device-scale semantics.
Other image sources, GPU surfaces, SVG documents, other filters, and CanvasKit
remain behind the same Fission contracts while their production implementations
are completed.

This crate deliberately reports only semantics that its current adapter can
honor. The initial foundation profile is not advertised as a complete
production graphical profile.

The default `skia-prebuilt` feature consumes Fission's verified native artifact.
`skia-build-from-source` is the explicit source-build path, while `test-shim`
exists only for ABI and ownership tests and is never a renderer profile.
