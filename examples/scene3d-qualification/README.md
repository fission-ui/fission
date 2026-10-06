# 3D qualification game

Beacon Run is the public end-to-end example for Fission's first 3D alpha. It
uses the same public APIs available to applications: `fission::game`,
`fission::scene3d`, `fission::physics`, and `fission::physics_rapier3d`.

The example demonstrates a retained scene with depth and three light types, a
static textured glTF model, renderer-independent viewport picking, a following
camera, a kinematic player that collides with fixed geometry, and a dynamic
cargo orb. Selecting and reaching the beacon completes the game. Ordinary
Fission buttons provide pointer, touch, keyboard-focus, accessibility, and
semantic-test activation; the game input map also maps device-independent keys
and scene gestures to the same messages.

Run from this directory so the portable `asset://assets/beacon.ppm` source has
the same relative shape on native and Web targets:

```sh
cd examples/scene3d-qualification
fission run --target linux --project-dir .
```

Use `macos`, `windows`, or `web` for the other initially qualified targets.
The asset provenance is recorded in [`assets/PROVENANCE.md`](assets/PROVENANCE.md).

The focused tests cover static import, picking, collision, snapshot restore,
and deterministic replay of a complete successful run.

On a graphical Linux, macOS, or Windows session, the ignored native
qualification launches the real desktop shell, selects the beacon by viewport
coordinate, completes the keyboard path, captures the rendered frame, and
rejects blank or compatibility-only pixels:

```sh
cargo test -p scene3d-qualification --test native_live -- --ignored --nocapture
```

With the Web app running, the ignored browser qualification performs real
coordinate-based touch picking, completes the run through browser keyboard
events, captures a screenshot, and rejects blank or compatibility-only pixels:

```sh
FISSION_SCENE3D_WEB_URL=http://127.0.0.1:8123 \
  cargo test -p scene3d-qualification --test web_live -- --ignored
```
