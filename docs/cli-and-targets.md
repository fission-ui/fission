# Fission CLI and target status

## Commands

Create a new app:

```sh
fission init my-app
```

Register an existing app or example without replacing its files:

```sh
fission init examples/web-smoke
```

`fission init` is non-destructive for non-empty directories. It preserves existing source, README, asset, and platform files; derives the package name from `Cargo.toml` when possible; detects existing `platforms/<target>/` folders; writes `fission.toml`; and creates only missing generated support files.

Create a new app against a local Fission checkout:

```sh
fission init my-app --local-path /path/to/fission
```

Add more platform targets:

```sh
cargo fission add-target web ios android --project-dir my-app
```

Diagnose local SDKs, emulators, browsers, and Rust targets:

```sh
cargo fission doctor web ios android --project-dir my-app
```

List the devices and runtime targets the CLI can launch:

```sh
cargo fission devices --project-dir my-app
cargo fission devices --project-dir my-app --json
```

Run an app on the selected target. The command attaches by default, so desktop stdout/stderr, web server requests, iOS simulator logs, or Android `logcat` output stay in the terminal until you stop them:

```sh
cargo fission run --project-dir my-app
cargo fission run --project-dir my-app --target web
cargo fission run --project-dir my-app --target ios --device <simulator-udid>
cargo fission run --project-dir my-app --target android --device emulator-5554
```

Start without attaching when you want the app to keep running in the background:

```sh
cargo fission run --project-dir my-app --target web --detach
cargo fission logs --project-dir my-app --target web --follow
```

Build or smoke-test a configured target:

```sh
cargo fission build --project-dir my-app --target web --release
cargo fission test --project-dir my-app --target web
cargo fission test --project-dir my-app --target ios --headless
cargo fission test --project-dir my-app --target android --headless
```

Package, check, and publish release artifacts:

```sh
cargo fission package --project-dir my-app --target site --format static --release
cargo fission package --project-dir my-app --target linux --format run --release
cargo fission package --project-dir my-app --target macos --format app --release
cargo fission package --project-dir my-app --target android --format apk --release
cargo fission readiness release --project-dir my-app --target site --format static --provider github-pages --site production
cargo fission readiness distribute --project-dir my-app --provider github-pages --site production --artifact my-app/target/fission/release/site/static/artifact-manifest.json
cargo fission distribute setup --project-dir my-app --provider github-pages --site production
cargo fission distribute --project-dir my-app --provider github-pages --site production --artifact my-app/target/fission/release/site/static/artifact-manifest.json
cargo fission distribute --project-dir my-app --provider play-store --track internal --artifact my-app/target/fission/release/android/aab/artifact-manifest.json
```

Every package command stages output under `target/fission/<profile>/<target>/<format>` and writes `artifact-manifest.json` with file hashes and MIME types. Static site/web publishing supports GitHub Pages, Cloudflare Pages, Netlify, direct S3-compatible object storage uploads through the Rust AWS SDK, and direct OAuth-backed uploads to Google Drive, OneDrive, and Dropbox. Store providers are represented in the lifecycle command surface so release metadata, beta groups, signing checks, review operations, and authentication can be validated from the same project root before provider-specific store APIs mutate remote state.

Release lifecycle commands are intentionally separate from packaging:

```sh
cargo fission release-config validate --project-dir my-app --provider play-store
cargo fission release-config add-release --project-dir my-app --version 1.2.3 --build 42 --yes
cargo fission release-content validate --project-dir my-app --provider app-store
cargo fission beta groups list --project-dir my-app --provider app-store
cargo fission signing status --project-dir my-app --target ios
cargo fission reviews list --project-dir my-app --provider play-store --since 30d
cargo fission auth status --json
```

The CLI keeps provider credentials out of `fission.toml`; auth commands can inspect environment-provided credentials or store imported secrets in an encrypted local vault whose key lives in the OS credential store.

The generated project contains:

- `src/main.rs` desktop entrypoint
- `src/lib.rs` shared desktop/mobile/web entry helpers
- `src/app.rs` minimal counter app
- `assets/app-icon.png` seeded from `docs/fission_logo.png`
- `fission.toml` target manifest
- `platforms/<target>/README.md` target notes and prerequisites
- target smoke scripts such as `platforms/web/test-browser.sh`, `platforms/ios/test-sim.sh`, and `platforms/android/test-emulator.sh`

## Verified flow

This branch has a verified scaffolding smoke path for desktop, web, iOS simulator, and Android emulator scaffolding:

```sh
cargo run -p fission-cli --bin fission -- init /tmp/demo-app --local-path "$PWD"
cargo run -p fission-cli --bin cargo-fission -- fission add-target web ios android --project-dir /tmp/demo-app
cargo run -p fission-cli --bin fission -- doctor web ios android --project-dir /tmp/demo-app
cd /tmp/demo-app
cargo check
```

Generated-app commands from the scaffolded project root:

```sh
cargo fission devices --project-dir .
cargo fission run --target web --project-dir .
cargo fission run --target ios --project-dir .
cargo fission run --target android --project-dir .
cargo fission test --target web --project-dir .
cargo fission test --target ios --project-dir .
cargo fission test --target android --project-dir .
```

The repository also keeps checked-in smoke examples:

```sh
./examples/mobile-smoke/platforms/ios/run-sim.sh
./examples/mobile-smoke/platforms/ios/test-sim.sh
./examples/mobile-smoke/platforms/android/run-emulator.sh
./examples/mobile-smoke/platforms/android/test-emulator.sh
./examples/web-smoke/platforms/web/run-browser.sh
./examples/web-smoke/platforms/web/test-browser.sh
```

## Current target status

| Target | Scaffolded by CLI | Compile smoke in repo | Runnable today | Notes |
|---|---|---:|---:|---|
| Windows | yes | yes | yes | Uses the generated desktop entrypoint |
| macOS | yes | yes | yes | Uses the generated desktop entrypoint |
| Linux | yes | yes | yes | Uses the generated desktop entrypoint |
| iOS | yes | yes | yes (simulator) | simulator app bundles are generated and can be health-checked through test control |
| Android | yes | yes | yes (emulator) | package scripts auto-detect SDK, NDK, toolchain, platform, and build-tools where possible |
| Web | yes | yes | yes (browser) | `wasm-pack` builds the app and `test-browser.sh` launches Chrome/Chromium with CDP enabled |

## Development workflow

The intended daily workflow is:

1. `cargo fission doctor --project-dir .` before starting platform work, especially on a new machine or CI runner.
2. `cargo fission devices --project-dir .` to see the local desktop target, Chrome/Chromium, Android devices/emulators, and iOS simulators.
3. `cargo fission run --target <target> --device <id> --project-dir .` while developing. Omit `--device` for the interactive selector when more than one runnable device exists.
4. `cargo fission run --target <target> --detach --project-dir .` when you want the launched app/server to keep running without owning the terminal.
5. `cargo fission logs --target <target> --device <id> --project-dir . --follow` to attach logs later.
6. `cargo fission build --target <target> --project-dir . --release` before producing artifacts for a tester.
7. `cargo fission test --target <target> --project-dir .` to run the generated platform smoke test.

Device ids are stable enough for scripts: Android uses the `adb` serial, iOS uses the simulator UDID, web uses `chrome`, and desktop uses `desktop`.

## Toolchains, env vars, and paths

Install the Rust targets:

```sh
rustup target add aarch64-apple-ios aarch64-apple-ios-sim aarch64-linux-android wasm32-unknown-unknown
```

Run doctor before platform work:

```sh
cargo fission doctor web ios android --project-dir .
```

### iOS

Required tools:

- macOS with Xcode installed
- iPhoneSimulator SDK visible through `xcrun --sdk iphonesimulator --show-sdk-path`
- Rust targets `aarch64-apple-ios` and `aarch64-apple-ios-sim`

Commands:

```sh
./examples/mobile-smoke/platforms/ios/run-sim.sh
./examples/mobile-smoke/platforms/ios/test-sim.sh
```

Generated app command after `cargo fission add-target ios`:

```sh
cargo fission run --target ios --project-dir .
cargo fission test --target ios --project-dir .
```

The generated iOS script opens the Simulator app by default. Set `IOS_SIM_HEADLESS=1` for CI or background-only runs.

### Android

Required tools:

- Android SDK
- Android NDK
- Rust target `aarch64-linux-android`

The generated package script detects `ANDROID_HOME`, the latest installed NDK, the matching NDK LLVM prebuilt host directory, the latest installed Android platform, and build-tools. Override these only when the auto-detected value is wrong:

- `ANDROID_HOME` or `ANDROID_SDK_ROOT`
- `ANDROID_NDK`
- `ANDROID_TOOLCHAIN`
- `ANDROID_MIN_API_LEVEL` (default: `24`)
- `ANDROID_TARGET_API_LEVEL` (default: latest installed platform)
- `ANDROID_BUILD_TOOLS`

Android emulator controls:

- `ANDROID_EMULATOR_HEADLESS=1` for background/CI runs
- `ANDROID_EMULATOR_RESTART=1` to kill an already-running hidden emulator and relaunch it visibly
- `ANDROID_EMULATOR_API_LEVEL`, `ANDROID_AVD_NAME`, or `ANDROID_SYSTEM_IMAGE` to pick a specific emulator runtime

Commands:

```sh
./examples/mobile-smoke/platforms/android/run-emulator.sh
./examples/mobile-smoke/platforms/android/test-emulator.sh
```

Generated app command after `cargo fission add-target android`:

```sh
cargo fission run --target android --project-dir .
cargo fission test --target android --project-dir .
```

### Web / WASM

Required tools:

```sh
rustup target add wasm32-unknown-unknown
cargo install wasm-pack
node --version # Node 22+ is required by the CDP smoke test
cargo fission doctor web --project-dir .
```

The browser test script uses Node.js plus Chrome/Chromium's DevTools Protocol endpoint. It starts a transient server, fails on browser runtime or console errors, and waits for a non-empty canvas. Set `FISSION_CHROME=/path/to/chrome` if the browser cannot be auto-detected.

Commands:

```sh
./examples/web-smoke/platforms/web/run-browser.sh
./examples/web-smoke/platforms/web/test-browser.sh
```

Generated app command after `cargo fission add-target web`:

```sh
cargo fission run --target web --project-dir .
cargo fission test --target web --project-dir .
```

Relevant paths:

- CLI: `crates/tools/fission-cli/`
- mobile shell: `crates/shell/fission-shell-mobile/`
- web shell: `crates/shell/fission-shell-web/`
- mobile smoke example: `examples/mobile-smoke/`
- web smoke example: `examples/web-smoke/`
- target scaffolding docs in generated apps: `platforms/<target>/README.md`

## Immediate next work

1. add browser-side semantic test control so web can use the same interaction tooling as desktop/mobile
2. validate iOS on physical devices after simulator coverage
3. extend generated host projects from smoke packaging toward release packaging
4. add first-party devtools hooks so the CLI can launch apps with widget-tree inspection enabled
