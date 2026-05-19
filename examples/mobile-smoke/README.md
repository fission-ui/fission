# mobile-smoke

Shared mobile smoke example for the current `fission-shell-mobile` path.

## What it proves

- the shared runtime launches on the host through `MobileApp`
- the same example packages and launches on the Android emulator
- the same example packages, launches, and renders on the iOS simulator through the software fallback path
- the packaged mobile targets use `docs/fission_logo.png` as the default app icon

## Commands

Desktop preview:

```sh
cargo run -p mobile-smoke
```

iOS simulator smoke:

```sh
rustup target add aarch64-apple-ios aarch64-apple-ios-sim
xcrun --sdk iphonesimulator --show-sdk-path
./examples/mobile-smoke/platforms/ios/run-sim.sh
./examples/mobile-smoke/platforms/ios/test-sim.sh
```

The simulator window opens by default. Set `IOS_SIM_HEADLESS=1` for CI or other background-only runs.

Optional test-control port:

```sh
FISSION_TEST_CONTROL_PORT=48711 ./examples/mobile-smoke/platforms/ios/run-sim.sh
curl http://127.0.0.1:48711/health
```

Renderer note:

- CoreSimulator still lacks `DownlevelFlags(INDIRECT_EXECUTION)`
- the shared shell now falls back to the software renderer in that case, so the simulator path stays usable

Android emulator smoke on macOS:

```sh
rustup target add aarch64-linux-android
export ANDROID_HOME="$HOME/Library/Android/sdk"
export ANDROID_MIN_API_LEVEL=24

./examples/mobile-smoke/platforms/android/run-emulator.sh
./examples/mobile-smoke/platforms/android/test-emulator.sh
```

The package script auto-detects the newest installed NDK, the matching NDK LLVM host toolchain, the latest installed Android platform, and build-tools. Use `cargo fission doctor android --project-dir examples/mobile-smoke` when your SDK layout needs explicit environment variables.

Android emulator controls:

- visible by default when the script has to boot a new AVD
- `ANDROID_EMULATOR_HEADLESS=1` for background/CI runs
- `ANDROID_EMULATOR_RESTART=1` to kill an already-running hidden emulator and relaunch it visibly
- `ANDROID_EMULATOR_API_LEVEL`, `ANDROID_AVD_NAME`, or `ANDROID_SYSTEM_IMAGE` to pick a specific emulator runtime
- `WGPU_BACKEND` defaults to `gl` on Android when unset so the emulator stays off the unstable Vulkan/SwiftShader path
- set `WGPU_BACKEND=vulkan` explicitly only if you want to audit that backend on a real device
- when `FISSION_TEST_CONTROL_PORT` is set, the Android shell keeps the event loop polling so semantic test-control commands stay responsive through `adb forward`
