# Beacon Run — 2D alpha qualification

This example is the public, functional qualification game for Fission's first
2D scene and deterministic game-runtime alpha. It uses only the public
`fission::scene2d`, `fission::game`, `fission::physics`, and
`fission::physics_rapier2d` APIs.

The player must navigate a world larger than the viewport, go around a static
collider, and reach the beacon. The camera follows the player. The retained
scene contains sprite, text, rectangle, and vector-path content. The on-scene
and ordinary Fission controls dispatch the same typed `MovePlayer` action for
pointer, touch, keyboard/accessibility activation, while the game input map
maps arrow keys and scene gestures to the same `GameMessage` values.

Run a native target:

```sh
fission run --target linux --project-dir examples/scene2d-qualification
```

Use `macos` or `windows` on those hosts.

Run the Web target:

```sh
fission run --target web --project-dir examples/scene2d-qualification
```

The headless tests cover collision, equivalent keyboard/pointer/accessible
input, a mid-run snapshot restore, and deterministic replay of a complete run.
On a graphical Linux, macOS, or Windows session, the ignored native
qualification launches the real desktop shell, completes the keyboard path,
captures the rendered frame, and rejects a blank or compatibility-only scene
viewport:

```sh
cargo test -p scene2d-qualification --test native_live -- --ignored --nocapture
```

With the Web app running, the ignored browser qualification drives separate
complete touch and keyboard runs, captures both results, and rejects a blank or
flat-colour scene viewport:

```sh
FISSION_SCENE2D_WEB_URL=http://127.0.0.1:8123 \
  cargo test -p scene2d-qualification --test web_live -- --ignored
```

## Asset provenance

`assets/scout-sheet.svg` is original vector artwork created for this example by
the Fission contributors on 2026-10-06. It is licensed under Apache-2.0; see
`assets/LICENSE.txt` and the repository root `LICENSE`. The exact source digest
is recorded in `assets/provenance.json` and in the scene's versioned asset
bundle.
