# fission-physics-rapier2d

Optional Rapier-backed implementation of Fission's renderer-neutral 2D physics
contract. Rapier types stay behind the provider boundary and are absent from an
application's dependency graph unless `physics-rapier2d` is enabled.

This crate is an **alpha API**. Use the exact prerelease requirement
`=0.1.0-alpha.1` when depending on it directly.
