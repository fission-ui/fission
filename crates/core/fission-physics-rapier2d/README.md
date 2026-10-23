# fission-physics-rapier2d

Optional Rapier-backed implementation of Fission's renderer-neutral 2D physics
contract. Rapier types stay behind the provider boundary and are absent from an
application's dependency graph unless `physics-rapier2d` is enabled.

This crate is an **alpha API**. Use the exact prerelease requirement
`=0.1.0-alpha.1` when depending on it directly.

Applications should normally enable `physics-rapier2d` on the `fission` facade. `RapierPhysicsWorld2D` implements `PhysicsProvider2D`, translates Fission bodies and shapes into Rapier resources, advances the world by the supplied fixed step, and maps contacts, queries and snapshots back to provider-neutral values.

```toml
[dependencies]
fission = { version = "0.15.1", features = ["physics-rapier2d"] }
```

Do not persist Rapier handles or expose them as game identity. `PhysicsBodyId` remains authoritative so the world can be recreated or another provider selected. Use the fixed-step runtime from `fission-game`; variable render-frame timing should not drive physics directly.

See the [scenes and games guide](https://fission.rs/docs/guides/scenes-and-games/) and `scene2d-qualification` for the maintained integration path.

## Licence

Apache-2.0.
