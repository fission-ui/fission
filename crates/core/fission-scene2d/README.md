# fission-scene2d

Alpha retained 2D scenes for Fission: sprites, images, text, vector paths,
batching, culling, picking, semantic interaction, and a normal Fission widget.

The API is functional but may change between alpha releases. Pin
`fission-scene2d = "=0.1.0-alpha.1"`. See the
[scenes and games guide](https://fission.rs/docs/guides/scenes-and-games/) and
the `scene2d-qualification` example in the Fission repository.

## Capabilities

`Scene2DIR` contains the versioned retained scene, camera, viewport, resources and nodes. Node content covers paths, images, text, sprites and batches. Processing validates references, resolves world transforms, culls invisible content and emits `Scene2DRenderPacket` for the renderer. Picking and `Interaction2D` connect scene nodes to semantic Fission actions without making the renderer own application state.

Applications normally enable `scene2d` through the facade:

```toml
[dependencies]
fission = { version = "0.15.1", features = ["scene2d"] }
```

`Scene2D` is a normal retained Fission widget and can sit beside forms, navigation and overlays. Use stable node and asset IDs, validate imported data before presentation, and treat rendering resources as rebuildable projections.

Run `cargo run -p scene2d-qualification` in the repository to exercise rendering, picking, culling and scene updates.

## Licence

Apache-2.0.
