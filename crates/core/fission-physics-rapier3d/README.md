# fission-physics-rapier3d

Optional Rapier-backed implementation of Fission's renderer-neutral 3D physics
contract, including the MVP kinematic character controller. Rapier types stay
behind the provider boundary and are absent unless `physics-rapier3d` is
enabled.

This crate is an **alpha API**. Use the exact prerelease requirement
`=0.1.0-alpha.1` when depending on it directly.

Applications should normally enable `physics-rapier3d` on the `fission` facade. `RapierPhysicsWorld3D` implements `PhysicsProvider3D`, translates Fission-owned bodies and shapes into Rapier resources, advances the fixed-step world and maps contacts, queries, character movement and snapshots back to provider-neutral values.

```toml
[dependencies]
fission = { version = "0.15.1", features = ["physics-rapier3d"] }
```

Rapier handles are implementation details, not durable game identity. Keep `PhysicsBodyId` values in application state, run physics from `fission-game`'s fixed simulation step and treat the scene as a presentation of the resulting state.

See the [scenes and games guide](https://fission.rs/docs/guides/scenes-and-games/) and `scene3d-qualification` for the maintained integration path and current limitations.

## Licence

Apache-2.0.
