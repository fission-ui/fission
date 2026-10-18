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

## Runtime model

Implement `Game` for the deterministic simulation state, choose a fixed `GameConfig`, and advance it through `GameRuntime`. `InputMap` translates host keyboard, pointer, controller and gesture events into semantic application actions. `DeterministicRandom` snapshots alongside game state so replay and headless tests receive the same random sequence.

`GameRecorder`, `GameReplay` and `GameTestHarness` make recorded input and snapshots testable without a window. `GameInputRegion` is the normal Fission widget boundary for host input; a 2D or 3D scene remains a presentation of the authoritative simulation.

Enable the facade feature rather than combining implementation crates manually:

```toml
[dependencies]
fission = { version = "0.15.1", features = ["game", "scene2d"] }
```

Physics is optional. When enabled, step the selected provider from the same fixed simulation clock and copy results into the game state intentionally.

## Licence

Apache-2.0.
