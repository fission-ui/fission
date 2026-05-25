# Fission command

This Cargo package installs the first-party `fission` command:

- `fission init`
- `fission add-target`
- `fission doctor`
- `fission devices`
- `fission run`
- `fission build`
- `fission test`
- `fission logs`
- `fission ...`

## Usage

Create a new app:

```sh
fission init my-app
```

Register an existing app or example without overwriting existing files:

```sh
fission init existing-app
```

When the directory already has source, docs, assets, or platform files, `init` preserves them. It derives the package name from `Cargo.toml` when possible, detects existing `platforms/<target>/` folders, writes `fission.toml`, and creates only missing generated support files.

Create a new app against a local Fission checkout:

```sh
fission init my-app --local-path /path/to/fission
```

Add more platform targets:

```sh
fission add-target web ios android --project-dir my-app
```

Check local SDKs, emulators, browsers, and Rust targets:

```sh
fission doctor web ios android --project-dir my-app
```

List runnable devices and targets:

```sh
fission devices --project-dir my-app
fission devices --project-dir my-app --json
```

Run and attach to app output/logs:

```sh
fission run --project-dir my-app
fission run --project-dir my-app --target web
fission run --project-dir my-app --target android --device emulator-5554
fission run --project-dir my-app --target ios --device <simulator-udid>
```

`fission run` attaches by default. Use `--detach` to start the app and return, then use `fission logs` to attach later where the platform supports it:

```sh
fission run --project-dir my-app --target web --detach
fission logs --project-dir my-app --target web --follow
```

Build or run smoke tests without launching the full attached workflow:

```sh
fission build --project-dir my-app --target web --release
fission test --project-dir my-app --target web
fission test --project-dir my-app --target ios --headless
```

## Current platform status

- `windows`, `macos`, `linux`: scaffolded and runnable through `fission run`
- `ios`: scaffolded by the CLI and runnable on the simulator through `fission run --target ios`
- `android`: scaffolded by the CLI and runnable on a device or emulator through `fission run --target android`
- `web`: scaffolded by the CLI and runnable in a browser through `fission run --target web`

The CLI writes platform state to `fission.toml` and creates `platforms/<target>/README.md` notes with the current prerequisites and next steps for each target. For iOS it also generates:

- `assets/app-icon.png` seeded from Fission's `docs/fission_logo.png`
- `platforms/ios/Info.plist`
- `platforms/ios/package-sim.sh`
- `platforms/ios/run-sim.sh`
- `platforms/ios/test-sim.sh`

For Android it also generates:

- `platforms/android/AndroidManifest.xml`
- `platforms/android/package-apk.sh`
- `platforms/android/run-emulator.sh`
- `platforms/android/test-emulator.sh`

For Web it also generates:

- `platforms/web/index.html`
- `platforms/web/bootstrap.mjs`
- `platforms/web/build-wasm.sh`
- `platforms/web/run-browser.sh`
- `platforms/web/test-browser.sh`

The generated iOS bundle, Android package, and browser host page all use `assets/app-icon.png` as the default app icon seed.

See also:

- `../../../docs/cli-and-targets.md`
- `../../../docs/platform-smoke-tests.md`
