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
| iOS | `examples/mobile-smoke` + `fission-shell-mobile` | runnable on simulator | CoreSimulator now falls back to the shared software renderer when Vello cannot use `INDIRECT_EXECUTION`; the app renders, responds to taps, and serves test control |
| Android | `examples/mobile-smoke` + `fission-shell-mobile` | runnable on emulator | requires Android SDK + NDK env vars; both the checked-in example and a CLI-generated app package, install, and launch through `run-emulator.sh` |
| Web/WASM | `examples/web-smoke` + `fission-shell-web` | runnable in browser | `wasm-pack` builds a real browser target, the checked-in `web-smoke` example serves locally, and CLI-generated apps now scaffold the same host project layout |

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

Launch command:

```sh
./examples/mobile-smoke/platforms/ios/run-sim.sh
```

Optional test-control port:

```sh
FISSION_TEST_CONTROL_PORT=48711 ./examples/mobile-smoke/platforms/ios/run-sim.sh
curl http://127.0.0.1:48711/health
```

What changed:

- CoreSimulator still lacks `DownlevelFlags(INDIRECT_EXECUTION)`
- `fission-shell-winit` now detects that case and falls back to the shared software renderer
- the simulator path now renders visible pixels, responds to tap input, and returns non-black screenshots through test control

Relevant paths:

- shell: `crates/shell/fission-shell-mobile/`
- example: `examples/mobile-smoke/`
- example iOS host files:
  - `examples/mobile-smoke/platforms/ios/Info.plist`
  - `examples/mobile-smoke/platforms/ios/package-sim.sh`
  - `examples/mobile-smoke/platforms/ios/run-sim.sh`

## Android

Required tools:

- Android SDK
- Android NDK

Verified environment on this branch:

```sh
export ANDROID_HOME="$HOME/Library/Android/sdk"
export ANDROID_NDK="$ANDROID_HOME/ndk/24.0.8215888"
export ANDROID_TOOLCHAIN="$ANDROID_NDK/toolchains/llvm/prebuilt/darwin-x86_64/bin"
export CC_aarch64_linux_android="$ANDROID_TOOLCHAIN/aarch64-linux-android24-clang"
export AR_aarch64_linux_android="$ANDROID_TOOLCHAIN/llvm-ar"
```

Smoke command:

```sh
./examples/mobile-smoke/platforms/android/run-emulator.sh
```

Optional test-control port:

```sh
FISSION_TEST_CONTROL_PORT=48761 ./examples/mobile-smoke/platforms/android/run-emulator.sh
curl http://127.0.0.1:48761/health
```

Notes:

- the host prebuilt directory may be `darwin-arm64`, `darwin-x86_64`, or a Linux variant on other machines
- the script launches a visible emulator when it boots a fresh AVD
- set `ANDROID_EMULATOR_HEADLESS=1` for background/CI runs
- set `ANDROID_EMULATOR_RESTART=1` if a hidden emulator is already running and you want the script to relaunch it visibly
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
```

Smoke command:

```sh
./examples/web-smoke/platforms/web/run-browser.sh
```

This script:

- builds the wasm package with `wasm-pack`
- serves the repository root at `http://127.0.0.1:8123/examples/web-smoke/platforms/web/`
- leaves the server in the foreground so you can open that URL in any browser

Relevant paths:

- shell: `crates/shell/fission-shell-web/`
- example: `examples/web-smoke/`

## Generated app smoke

A newly scaffolded app was also verified in this branch:

```sh
fission init /tmp/demo-app --local-path "$PWD"
cargo fission add-target ios android web --project-dir /tmp/demo-app
cd /tmp/demo-app
./platforms/ios/run-sim.sh
# after exporting the Android env block from the Android section above
./platforms/android/run-emulator.sh
./platforms/web/run-browser.sh
```

The generated app now gets:

- iOS simulator packaging and launch scripts
- Android launcher/package/install scripts
- a real web host page plus `wasm-pack` build and local browser serve scripts
