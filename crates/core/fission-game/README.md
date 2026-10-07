# fission-game

Deterministic fixed-step simulation, semantic input, snapshots, replay, and
headless control shared by Fission 2D and 3D games.

This crate is an **alpha API**. Enable the facade's `game` feature and import
the intended long-term path:

```rust
use fission::game::*;
```

The game state and clock remain authoritative; renderers and optional physics
providers do not own application state. See the
[Scenes and games guide](https://fission.rs/docs/guides/scenes-and-games/).
