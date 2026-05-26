# fission-3d

3D scene primitives for Fission.

`fission-3d` provides the data structures used by Fission widgets that embed simple 3D scenes in the normal UI tree. It is usually consumed through the `fission` facade with the `three-d` feature enabled:

```toml
[dependencies]
fission = { version = "0.2.0", features = ["desktop", "three-d"] }
```

Use this crate directly only when you are extending the framework's 3D model or renderer integration.

## What it contains

- Scene, camera, mesh, material, light, and transform data structures.
- Serialization-friendly scene descriptions that can travel through the same widget and rendering pipeline as other Fission nodes.
- GPU-oriented vertex and material representations used by shell renderers.

## Design notes

Fission treats 3D as part of the UI surface, not as a separate application loop. A 3D widget participates in layout, input, semantics, and rendering like any other widget. The shell decides how to compose the scene with the rest of the frame.

## Documentation

See [fission.rs](https://fission.rs) for guides and examples covering embeds, media, and 3D scenes.

## License

MIT
