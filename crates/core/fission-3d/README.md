# fission-3d

3D scene primitives for Fission.

`fission-3d` provides the data structures used by Fission widgets that embed simple 3D scenes in the normal UI tree. It is usually consumed through the `fission` facade with the `three-d` feature enabled:

```toml
[dependencies]
fission = { version = "0.11.1", features = ["desktop", "three-d"] }
```

Use this crate directly only when you are extending the framework's 3D model or renderer integration.

## What it contains

- The `Scene3D` widget and its existing builder API.
- Serialization-friendly points and primitives re-exported from the neutral
  `fission-3d-model` crate.

Renderer integrations are deliberately separate. The current wgpu
implementation lives in `fission-render-wgpu3d` and consumes
`fission-3d-model`; applications continue to use the `Scene3D` widget instead
of constructing a renderer.

The `fission_3d::render` module preserves the crate's existing public renderer
exports. Backend-neutral consumers should use `fission-3d-model` directly;
Fission's host integrations now consume the neutral model and renderer adapter
as separate dependencies.

## Design notes

Fission treats 3D as part of the UI surface, not as a separate application
loop. A 3D widget participates in layout, input, semantics, and rendering like
any other widget. Fission-owned model types do not expose wgpu; the selected
general-GPU adapter and shell decide how to render and compose the scene.

## Documentation

See [fission.rs](https://fission.rs) for guides and examples covering embeds, media, and 3D scenes.

## License

MIT
