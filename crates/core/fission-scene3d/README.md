# fission-scene3d

Alpha retained 3D scene contracts for Fission. The crate provides a closed,
serialisable scene IR, validation and transform/culling preparation, retained
resource accounting, static glTF/GLB import, and renderer-independent picking.

The first alpha deliberately supports static scenes only. Skeletal animation,
morph targets, shadows, spot lights, custom shaders, and post-processing are
outside its current contract.

Pin `fission-scene3d = "=0.1.0-alpha.1"` when depending on it directly. See
the [scenes and games guide](https://fission.rs/docs/guides/scenes-and-games/)
and the `scene3d-qualification` example in the Fission repository.

## Capabilities

`Scene3DIR` contains a versioned camera, viewport, nodes, meshes, materials, textures and lights. Validation and processing resolve transforms, bounds and visible resources before producing a renderer-independent packet. `Ray3D` and `PickHit3D` support deterministic picking against the retained scene.

`GltfImporter` accepts static glTF/GLB content and returns Fission-owned scene resources plus imported textures. Validate import diagnostics and keep source provenance in the asset layer. The importer does not turn external animation or shader extensions into partially supported behaviour.

Applications normally enable the facade feature:

```toml
[dependencies]
fission = { version = "0.15.1", features = ["scene3d"] }
```

`Scene3D` remains a normal Fission widget, so the surrounding application owns routes, state, forms, overlays and accessibility. Run `cargo run -p scene3d-qualification` in the repository for the maintained integration workload.

## Licence

Apache-2.0.
