# fission-scene

Shared renderer-neutral identities, assets, mathematics, diagnostics, and pass
accounting for Fission's retained 2D and 3D scenes.

This crate is an **alpha API**. Its closed, versioned data contracts reject
unsupported format versions rather than guessing. Most applications should use
it through `fission::scene2d` or `fission::scene3d`.

See the [Scenes and games guide](https://fission.rs/docs/guides/scenes-and-games/)
for supported workflows and current limits.
