# fission-physics-rapier3d

Optional Rapier-backed implementation of Fission's renderer-neutral 3D physics
contract, including the MVP kinematic character controller. Rapier types stay
behind the provider boundary and are absent unless `physics-rapier3d` is
enabled.

This crate is an **alpha API**. Use the exact prerelease requirement
`=0.1.0-alpha.1` when depending on it directly.
