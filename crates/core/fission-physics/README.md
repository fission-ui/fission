# fission-physics

Alpha provider-neutral 2D and 3D physics contracts for Fission. Stable body
IDs, Fission-owned shapes and values, queries, contact and trigger transitions,
fixed-step simulation, and supported snapshot restore stay independent of a
particular solver.

The API may change between alpha releases. Pin
`fission-physics = "=0.1.0-alpha.1"`. Rapier implementations are optional and
independently selectable through `fission-physics-rapier2d` and
`fission-physics-rapier3d`.
