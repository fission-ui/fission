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
