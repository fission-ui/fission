# fission-physics

Alpha provider-neutral 2D and 3D physics contracts for Fission. Stable body
IDs, Fission-owned shapes and values, queries, contact and trigger transitions,
fixed-step simulation, and supported snapshot restore stay independent of a
particular solver.

The API may change between alpha releases. Pin
`fission-physics = "=0.1.0-alpha.1"`. Rapier implementations are optional and
independently selectable through `fission-physics-rapier2d` and
`fission-physics-rapier3d`.

## Provider contract

`PhysicsProvider2D` and `PhysicsProvider3D` operate on Fission-owned bodies, colliders, poses, velocities and query filters. Stable `PhysicsBodyId` values keep game state independent of a solver's internal handles. Providers report contact and trigger transitions explicitly and support versioned snapshots where restoration is advertised.

The facade is the normal application dependency:

```toml
[dependencies]
fission = { version = "0.15.1", features = ["physics-rapier2d"] }
```

Physics does not own the fixed-step clock. Drive it from `fission-game`'s simulation step, keep gameplay state authoritative and copy the bounded results needed by scene presentation. Select either dimension deliberately; enabling physics does not silently add both Rapier implementations.

See the [scenes and games guide](https://fission.rs/docs/guides/scenes-and-games/) and the scene qualification examples for integration patterns.

## Licence

Apache-2.0.
