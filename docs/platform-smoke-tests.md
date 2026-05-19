# Platform smoke tests

This document records the current reproducible target setup for the Fission repository.

If you cloned the repo fresh, initialize submodules first:

```sh
git submodule update --init --recursive
```

## Current status

| Target | Example / shell | Status | Notes |
|---|---|---|---|
| Desktop | `examples/mobile-smoke` + `fission-shell-mobile` | runnable | `cargo run -p mobile-smoke` uses the shared winit + Vello path on the host |
| iOS | `examples/mobile-smoke` + `fission-shell-mobile` | runnable on simulator | package, launch, and health-check scripts run through `simctl` and the test-control port |
| Android | `examples/mobile-smoke` + `fission-shell-mobile` | runnable on emulator | package scripts auto-detect SDK, NDK, toolchain, platform, and build-tools where possible |
| Web/WASM | `examples/web-smoke` + `fission-shell-web` | runnable in browser | `wasm-pack` builds a real browser target and the CDP smoke script launches Chrome/Chromium headlessly, fails on browser runtime errors, and waits for a rendered canvas |

## Doctor

Use the CLI doctor before platform work:

```sh
cargo run -p fission-cli --bin fission -- doctor web ios android --project-dir examples/mobile-smoke
```

For a generated app, run:

```sh
cargo fission doctor web ios android --project-dir .
```

Doctor checks Rust targets, wasm-pack, Node.js CDP support, Chrome/Chromium, Xcode/simctl, Android SDK tools, installed Android platforms, build-tools, NDK, and the NDK clang linker for the selected Android minimum API.

## Rust targets

Install the Rust targets once:

```sh
rustup target add aarch64-apple-ios aarch64-apple-ios-sim aarch64-linux-android wasm32-unknown-unknown
```

## iOS

Required tools:

- Xcode
- iPhoneSimulator SDK visible through `xcrun`

Sanity check:

```sh
xcrun --sdk iphonesimulator --show-sdk-path
```

Launch and smoke-test commands:

```sh
./examples/mobile-smoke/platforms/ios/run-sim.sh
./examples/mobile-smoke/platforms/ios/test-sim.sh
```

The script opens the Simulator app by default so you can see the app launch. Set `IOS_SIM_HEADLESS=1` for CI or background-only runs.

Optional manual test-control check:

```sh
FISSION_TEST_CONTROL_PORT=48711 ./examples/mobile-smoke/platforms/ios/run-sim.sh
curl http://127.0.0.1:48711/health
```

Relevant paths:

- shell: `crates/shell/fission-shell-mobile/`
- example: `examples/mobile-smoke/`
- example iOS host files:
  - `examples/mobile-smoke/platforms/ios/Info.plist`
  - `examples/mobile-smoke/platforms/ios/package-sim.sh`
  - `examples/mobile-smoke/platforms/ios/run-sim.sh`
  - `examples/mobile-smoke/platforms/ios/test-sim.sh`

## Android

Required tools:

- Android SDK
- Android NDK
- Android emulator package
- Rust target `aarch64-linux-android`

The package script auto-detects the newest installed NDK, the correct NDK LLVM prebuilt host directory, the latest installed Android platform, and build-tools. Use these environment variables only when you need explicit control:

```sh
export ANDROID_HOME="$HOME/Library/Android/sdk"        # or ANDROID_SDK_ROOT
export ANDROID_MIN_API_LEVEL=24                         # clang min API and manifest minSdk
export ANDROID_TARGET_API_LEVEL=35                      # manifest targetSdk and android.jar
export ANDROID_NDK="$ANDROID_HOME/ndk/<version>"        # optional explicit NDK
export ANDROID_TOOLCHAIN="$ANDROID_NDK/toolchains/llvm/prebuilt/<host>/bin"
export ANDROID_BUILD_TOOLS=35.0.0                       # optional version or path
```

Smoke commands:

```sh
./examples/mobile-smoke/platforms/android/run-emulator.sh
./examples/mobile-smoke/platforms/android/test-emulator.sh
```

Optional manual test-control check:

```sh
FISSION_TEST_CONTROL_PORT=48761 ./examples/mobile-smoke/platforms/android/run-emulator.sh
curl http://127.0.0.1:48761/health
```

Notes:

- the host prebuilt directory can differ by SDK installation and host OS
- the script launches a visible emulator when it boots a fresh AVD
- set `ANDROID_EMULATOR_HEADLESS=1` for background/CI runs
- set `ANDROID_EMULATOR_RESTART=1` if a hidden emulator is already running and you want the script to relaunch it visibly
- set `ANDROID_EMULATOR_API_LEVEL`, `ANDROID_AVD_NAME`, or `ANDROID_SYSTEM_IMAGE` to choose a specific emulator image
- `fission-shell-winit` forces `WGPU_BACKEND=gl` on Android when `WGPU_BACKEND` is unset so the emulator avoids the unstable Vulkan/SwiftShader path
- set `WGPU_BACKEND=vulkan` explicitly only if you are auditing that backend on a real device
- when `FISSION_TEST_CONTROL_PORT` is set, the Android shell keeps the event loop polling so `GetText`, `TapText`, and screenshot commands stay responsive through `adb forward`

Relevant paths:

- shell: `crates/shell/fission-shell-mobile/`
- example: `examples/mobile-smoke/`

## Web / WASM

Required tools:

```sh
rustup target add wasm32-unknown-unknown
cargo install wasm-pack
node --version # Node 22+ is required by the CDP smoke test
```

Smoke commands:

```sh
./examples/web-smoke/platforms/web/run-browser.sh
./examples/web-smoke/platforms/web/test-browser.sh
```

The run script builds the wasm package and keeps serving the repository root at:

- `http://127.0.0.1:8123/examples/web-smoke/platforms/web/`

The test script starts a transient local server, launches Chrome/Chromium headlessly with a DevTools Protocol port, fails on browser runtime or console errors, and waits for a non-empty canvas. It stops the server when the test exits. Set `FISSION_CHROME=/path/to/chrome` if Chrome cannot be auto-detected.

Relevant paths:

- shell: `crates/shell/fission-shell-web/`
- example: `examples/web-smoke/`

## Generated app smoke

A newly scaffolded app uses the same scripts:

```sh
fission init /tmp/demo-app --local-path "$PWD"
cargo fission add-target ios android web --project-dir /tmp/demo-app
cargo fission doctor web ios android --project-dir /tmp/demo-app
cd /tmp/demo-app
./platforms/ios/run-sim.sh
./platforms/ios/test-sim.sh
./platforms/android/run-emulator.sh
./platforms/android/test-emulator.sh
./platforms/web/run-browser.sh
./platforms/web/test-browser.sh
```

The generated app gets:

- iOS simulator packaging, launch, and health-check scripts
- Android package, install, launch, and health-check scripts
- a web host page plus `wasm-pack` build, local serve, and Chrome/Chromium CDP smoke scripts
