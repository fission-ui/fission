# fission-cli

`fission-cli` provides the first-party Fission project scaffolding commands:

- `fission init`
- `fission add-target`
- `cargo fission ...`

## Usage

Create a new app:

```sh
fission init my-app
```

Create a new app against a local Fission checkout:

```sh
fission init my-app --local-path /path/to/fission
```

Add more platform targets:

```sh
cargo fission add-target web ios android --project-dir my-app
```

## Current platform status

- `windows`, `macos`, `linux`: scaffolded and runnable through the generated desktop entrypoint
- `ios`: scaffolded by the CLI and runnable on the simulator through `platforms/ios/run-sim.sh`
- `android`: scaffolded by the CLI and runnable on the emulator through `platforms/android/run-emulator.sh`
- `web`: scaffolded by the CLI and runnable in a browser through `platforms/web/run-browser.sh`

The CLI writes platform state to `fission.toml` and creates `platforms/<target>/README.md` notes with the current prerequisites and next steps for each target. For iOS it also generates:

- `assets/app-icon.png` seeded from Fission's `docs/fission_logo.png`
- `platforms/ios/Info.plist`
- `platforms/ios/package-sim.sh`
- `platforms/ios/run-sim.sh`

For Android it also generates:

- `platforms/android/AndroidManifest.xml`
- `platforms/android/package-apk.sh`
- `platforms/android/run-emulator.sh`

For Web it also generates:

- `platforms/web/index.html`
- `platforms/web/bootstrap.mjs`
- `platforms/web/build-wasm.sh`
- `platforms/web/run-browser.sh`

The generated iOS bundle, Android package, and browser host page all use `assets/app-icon.png` as the default app icon seed.

See also:

- `../../../docs/cli-and-targets.md`
- `../../../docs/platform-smoke-tests.md`
