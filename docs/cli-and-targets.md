# Fission command and target status

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
fission add-target web ios android --project-dir my-app
```

Diagnose local SDKs, emulators, browsers, and Rust targets:

```sh
fission doctor web ios android --project-dir my-app
```

List the devices and runtime targets the CLI can launch:

```sh
fission devices --project-dir my-app
fission devices --project-dir my-app --json
```

Run an app on the selected target. The command attaches by default, so desktop stdout/stderr, web server requests, iOS simulator logs, or Android `logcat` output stay in the terminal until you stop them:

```sh
fission run --project-dir my-app
fission run --project-dir my-app --target web
fission run --project-dir my-app --target ios --device <simulator-udid>
fission run --project-dir my-app --target android --device emulator-5554
```

Start without attaching when you want the app to keep running in the background:

```sh
fission run --project-dir my-app --target web --detach
fission logs --project-dir my-app --target web --follow
```

Build or smoke-test a configured target:

```sh
fission build --project-dir my-app --target web --release
fission test --project-dir my-app --target web
fission test --project-dir my-app --target ios --headless
fission test --project-dir my-app --target android --headless
```

Package, check, and publish release artifacts:

```sh
fission package --project-dir my-app --target static-site --format static --release
fission package --project-dir my-app --target ssr --format docker-image --release
fission package --project-dir my-app --target linux --format run --release
fission package --project-dir my-app --target terminal --format run --release
fission package --project-dir my-app --target macos --format app --release
fission package --project-dir my-app --target android --format apk --release
fission readiness package --project-dir my-app --target android --format aab --release
fission readiness release --project-dir my-app --target static-site --format static --provider github-pages --site production --locale en-US
fission readiness distribute --project-dir my-app --provider github-pages --site production --artifact my-app/target/fission/release/static-site/static/artifact-manifest.json
fission distribute setup --project-dir my-app --provider github-pages --site production
fission distribute --project-dir my-app --provider github-pages --site production --artifact my-app/target/fission/release/static-site/static/artifact-manifest.json
fission distribute --project-dir my-app --provider play-store --track internal --locale en-US --artifact my-app/target/fission/release/android/aab/artifact-manifest.json
fission distribute --project-dir my-app --provider play-store --track internal-sharing --artifact my-app/target/fission/release/android/aab/artifact-manifest.json
```

Every package command stages output under `target/fission/<profile>/<target>/<format>` and writes `artifact-manifest.json` with file hashes, MIME types, version/build facts, validation checks, signing/notarization state where relevant, source configuration hashes, secondary debug-symbol/crash-diagnostic artifacts, and an icon manifest reference for targets that use application icons. Linux and Terminal `.run` packages include an embedded `--verify`/`--install`/`--uninstall` flow; packaging runs that flow in a temporary install root and records `package-validation/install-smoke.json` before the artifact manifest is finalized. Static package output records `package-validation/load-smoke.json` after validating the package root and `index.html`. macOS `.app`/`.pkg` artifacts resolve `CFBundleShortVersionString` and `CFBundleVersion` from `[package.macos]` or the active release, and macOS `.app`, Android APK/AAB, iOS IPA, and Windows MSIX artifacts also get deterministic structure validation before provider readiness can treat the manifest as distributable. Static site/web publishing supports GitHub Pages, Cloudflare Pages, Netlify, direct S3-compatible object storage uploads through the Rust AWS SDK, and direct OAuth-backed uploads to Google Drive, OneDrive, and Dropbox. Distribution readiness verifies artifact hashes, source configuration hashes, and debug-symbol/crash-asset upload coverage so stale or support-incomplete artifacts are visible before publish. GitHub Releases and object/file distribution default to safe duplicate/conflict behavior unless replacement or overwrite behavior is explicitly configured. Store providers are represented in the lifecycle command surface so release metadata, beta groups, signing checks, review operations, authentication, and store package versions can be validated from the same project root before provider-specific store APIs mutate remote state.

Readiness commands print the report in human or JSON form, but a blocked report still exits non-zero. Package readiness also reports whether any existing `artifact-manifest.json` is stale relative to the current source configuration, and distribution readiness fails stale artifacts before mutating a provider. CI can therefore parse `--json` output without treating provider-required failures as successful command runs.

Release lifecycle commands are intentionally separate from packaging. `release-config set` writes typed TOML scalars for known numeric/boolean release fields, so version codes, build numbers, and overwrite flags are not accidentally stored as strings:

```sh
fission release-config validate --project-dir my-app --provider play-store
fission release-config set app.version 1.2.3 --project-dir my-app --dry-run --json
fission release-config set app.build 42 --project-dir my-app --yes
fission release-config import --project-dir my-app --provider play-store --locales en-US --dry-run --json
fission release-config lock --project-dir my-app --provider play-store --locales en-US --yes
fission release-config write-file --project-dir my-app --release 1.2.3+42 --kind notes --locale en-US --from-file notes/en-US.md --dry-run --json
fission release-config add-release --project-dir my-app --version 1.2.3 --build 42 --yes
fission release-config bump-build --project-dir my-app --target android --yes
fission release-config version-state --project-dir my-app --provider play-store --target android --track internal --json
fission release-config push --project-dir my-app --provider play-store --locales en-US --yes
fission release-config skip-requirement --project-dir my-app --id release_content.play_store.feature_graphic --yes
fission release-content validate --project-dir my-app --provider app-store
fission beta groups list --project-dir my-app --provider app-store
fission signing status --project-dir my-app --target ios
fission reviews list --project-dir my-app --provider play-store --since 30d
fission release-workflow list --project-dir my-app --json
fission release-workflow run beta --project-dir my-app --dry-run --json
fission release-workflow run beta --project-dir my-app --yes
fission auth login play-store --json
fission auth status --json
fission auth logout app-store --json
fission auth import play-store --from PLAY_STORE_SERVICE_ACCOUNT_JSON_BASE64 --json
fission auth rotate microsoft-store --json
```

For direct non-interactive publishing, `fission publish --provider ... --yes`
uses the same provider defaults as the guided flow when `--artifact`,
`--target`, or `--format` are omitted. For example, Play Store defaults to
Android AAB/internal, App Store defaults to iOS IPA/TestFlight, Microsoft Store
defaults to Windows MSIX/private, static hosting defaults to static-site/static, and
Docker registry defaults to ssr/docker-image. When an artifact manifest is
supplied, its recorded target and format win.

For local interactive publishing, `fission publish --provider ...` opens the
terminal wizard and `fission publish --provider ... --app` opens the native
windowed wizard. `--app` is not a CI mode and is rejected with
`--guided`, `--dry-run`, `--yes`, `--json`, or `--overwrite-remote` so the
selected shell cannot silently ignore automation flags.

Store metadata pushes require a fresh provider baseline lock. Run
`fission release-config import ... --yes` or
`fission release-config lock ... --yes` after reviewing remote metadata; direct
publish and `release-config push` then verify that provider metadata has not
changed before mutating it. If the intended operation is to replace current
provider metadata after a manual review, pass `--overwrite-remote` explicitly.

The CLI keeps provider credentials out of `fission.toml`. Local and CI release flows must resolve secrets from environment variables, base64 environment variables decoded into secure temporary files that are cleaned up after use, provider-owned CLI auth state, or platform-owned certificate/key stores. Fission does not use a hidden project vault for release credentials. Interactive local flows may copy or move selected secret files into `~/.fission/<app-name>/`, but they refuse to keep live references to secret files inside the project tree.

The generated project contains:

- `src/main.rs` desktop entrypoint
- `src/lib.rs` shared desktop/mobile/web entry helpers
- `src/app.rs` minimal counter app
- `assets/app-icon.png` seeded from `docs/fission_logo.png`
- `fission.toml` target manifest with `[app]` version/build defaults plus target package/distribution defaults that reference environment-variable names rather than secret files
- `platforms/<target>/README.md` target notes and prerequisites
- target smoke scripts such as `platforms/web/test-browser.sh`, `platforms/ios/test-sim.sh`, and `platforms/android/test-emulator.sh`

## Verified flow

This branch has a verified scaffolding smoke path for desktop, web, iOS simulator, and Android emulator scaffolding. From a checkout where the `fission` command is installed or otherwise on `PATH`:

```sh
fission init /tmp/demo-app --local-path "$PWD"
fission add-target web ios android --project-dir /tmp/demo-app
fission doctor web ios android --project-dir /tmp/demo-app
cd /tmp/demo-app
cargo check
```

Generated-app commands from the scaffolded project root:

```sh
fission devices --project-dir .
fission run --target web --project-dir .
fission run --target ios --project-dir .
fission run --target android --project-dir .
fission test --target web --project-dir .
fission test --target ios --project-dir .
fission test --target android --project-dir .
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
| Windows | yes | yes | yes | Uses the generated desktop entrypoint; generated release targets include MSIX and MSI package scripts plus a full-trust MSIX manifest |
| macOS | yes | yes | yes | Uses the generated desktop entrypoint |
| Linux | yes | yes | yes | Uses the generated desktop entrypoint |
| iOS | yes | yes | yes (simulator) | simulator app bundles can be health-checked through test control; generated release targets include signed IPA packaging via explicit signing/provisioning inputs |
| Android | yes | yes | yes (emulator) | APK and AAB package scripts auto-detect SDK, NDK, toolchain, platform, build-tools, and env/base64 signing inputs where possible |
| Web | yes | yes | yes (browser) | `wasm-pack` builds the app and `test-browser.sh` launches Chrome/Chromium with CDP enabled |
| Static site | yes | yes | yes (local server) | `fission site build/serve` renders content and `static-site/static` packages the output |
| SSR | yes | yes | yes (local server) | `fission server serve` runs the server shell and `ssr/docker-image` packages deployment images |
| Terminal | yes | yes | yes | uses the terminal shell through the project Rust entrypoint; richer package formats are still terminal-RFC-owned |

## Development workflow

The intended daily workflow is:

1. `fission doctor --project-dir .` before starting platform work, especially on a new machine or CI runner.
2. `fission devices --project-dir .` to see the local desktop target, Chrome/Chromium, Android devices/emulators, and iOS simulators.
3. `fission run --target <target> --device <id> --project-dir .` while developing. Omit `--device` for the interactive selector when more than one runnable device exists.
4. `fission run --target <target> --detach --project-dir .` when you want the launched app/server to keep running without owning the terminal.
5. `fission logs --target <target> --device <id> --project-dir . --follow` to attach logs later.
6. `fission build --target <target> --project-dir . --release` before producing artifacts for a tester.
7. `fission test --target <target> --project-dir .` to run the generated platform smoke test.

Device ids are stable enough for scripts: Android uses the `adb` serial, iOS uses the simulator UDID, web uses `chrome`, and desktop uses `desktop`.

## Toolchains, env vars, and paths

Install the Rust targets:

```sh
rustup target add aarch64-apple-ios aarch64-apple-ios-sim aarch64-linux-android wasm32-unknown-unknown
```

Run doctor before platform work:

```sh
fission doctor web ios android --project-dir .
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

Generated app command after `fission add-target ios`:

```sh
fission run --target ios --project-dir .
fission test --target ios --project-dir .
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

Generated app command after `fission add-target android`:

```sh
fission run --target android --project-dir .
fission test --target android --project-dir .
```

### Web / WASM

Required tools:

```sh
rustup target add wasm32-unknown-unknown
cargo install wasm-pack
node --version # Node 22+ is required by the CDP smoke test
fission doctor web --project-dir .
```

The browser test script uses Node.js plus Chrome/Chromium's DevTools Protocol endpoint. It starts a transient server, fails on browser runtime or console errors, and waits for a non-empty canvas. Set `FISSION_CHROME=/path/to/chrome` if the browser cannot be auto-detected.

Commands:

```sh
./examples/web-smoke/platforms/web/run-browser.sh
./examples/web-smoke/platforms/web/test-browser.sh
```

Generated app command after `fission add-target web`:

```sh
fission run --target web --project-dir .
fission test --target web --project-dir .
```

Relevant paths:

- command package: `crates/tools/cargo-fission/`
- command implementation crates: `crates/tools/fission-command-*`
- mobile shell: `crates/shell/fission-shell-mobile/`
- web shell: `crates/shell/fission-shell-web/`
- mobile smoke example: `examples/mobile-smoke/`
- web smoke example: `examples/web-smoke/`
- target scaffolding docs in generated apps: `platforms/<target>/README.md`

## Immediate next work

1. add browser-side semantic test control so web can use the same interaction tooling as desktop/mobile
2. validate iOS on physical devices after simulator coverage
3. validate generated Windows MSI/MSIX packages on a Windows runner with real WiX, makeappx, and SignTool installed
4. add first-party devtools hooks so the CLI can launch apps with widget-tree inspection enabled
