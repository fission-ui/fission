# fission-render-skia

`fission-render-skia` is Fission's safe renderer adapter for the framework-owned
Skia ABI. It translates backend-neutral `InteractiveFrame` submissions into
batched Skia work without exposing Skia objects to applications, widgets,
layout, or platform hosts.

The first implementation profile is deterministic headless Skia raster. It
provides explicit lifecycle, readback, recovery, memory-pressure, diagnostic,
thread-affinity, and teardown behavior. GPU surfaces, SkParagraph, resources,
and CanvasKit are added behind the same Fission contracts as their production
implementations become available.

This crate deliberately reports only semantics that its current adapter can
honor. The initial foundation profile is not advertised as a complete
production graphical profile.

The default `skia-prebuilt` feature consumes Fission's verified native artifact.
`skia-build-from-source` is the explicit source-build path, while `test-shim`
exists only for ABI and ownership tests and is never a renderer profile.
