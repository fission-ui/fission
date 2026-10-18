# fission-scene

Shared renderer-neutral identities, assets, mathematics, diagnostics, and pass
accounting for Fission's retained 2D and 3D scenes.

This crate is an **alpha API**. Its closed, versioned data contracts reject
unsupported format versions rather than guessing. Most applications should use
it through `fission::scene2d` or `fission::scene3d`.

See the [Scenes and games guide](https://fission.rs/docs/guides/scenes-and-games/)
for supported workflows and current limits.

## Shared contracts

The crate owns stable `SceneId`, `NodeId`, `PresentationId` and `AssetId` values; renderer-neutral 2D/3D mathematics; typed asset handles and load state; structured diagnostics; and render-pass statistics. `SCENE_FORMAT_VERSION` is part of the serialised contract. Unknown versions are rejected rather than partially interpreted.

Most application code should enable `scene2d` or `scene3d` on the `fission` facade. Use this crate directly when implementing a scene importer, renderer bridge or tool that genuinely spans both dimensions. It does not render pixels, run a game loop or choose a physics provider.

Scene resources are retained data, while application or game state remains authoritative. Keep durable domain identity outside renderer caches so a render resource can be rebuilt without changing product state.

## Licence

Apache-2.0.
