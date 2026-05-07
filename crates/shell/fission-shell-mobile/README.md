# fission-shell-mobile

Mobile shell for the Fission UI framework (iOS and Android).

`fission-shell-mobile` provides the current mobile bootstrap layer for running Fission
applications on iOS and Android. In this branch it is backed by the shared
`fission-shell-winit` runtime while the first dedicated mobile lifecycle and packaging
work is being built out.

## Status

Current branch status:

- host desktop preview: verified
- Android emulator smoke: verified with the Android SDK + NDK env configured
- iOS simulator smoke: verified with the software renderer fallback
- touch, safe-area, soft-keyboard, and mobile-specific lifecycle hooks: still in progress

## Verified commands

Desktop preview of the shared UI path:

```sh
cargo run -p mobile-smoke
```

iOS simulator smoke:

```sh
rustup target add aarch64-apple-ios aarch64-apple-ios-sim
xcrun --sdk iphonesimulator --show-sdk-path
./examples/mobile-smoke/platforms/ios/run-sim.sh
```

Renderer note:

- CoreSimulator still does not expose `DownlevelFlags(INDIRECT_EXECUTION)`
- the shared shell now falls back to the software renderer in that case, so the simulator path stays usable while Vello remains the default elsewhere

Android emulator smoke on macOS:

```sh
rustup target add aarch64-linux-android
export ANDROID_HOME="$HOME/Library/Android/sdk"
export ANDROID_NDK="$ANDROID_HOME/ndk/24.0.8215888"
export ANDROID_TOOLCHAIN="$ANDROID_NDK/toolchains/llvm/prebuilt/darwin-x86_64/bin"
export CC_aarch64_linux_android="$ANDROID_TOOLCHAIN/aarch64-linux-android24-clang"
export AR_aarch64_linux_android="$ANDROID_TOOLCHAIN/llvm-ar"

./examples/mobile-smoke/platforms/android/run-emulator.sh
```

If your NDK uses a different host prebuilt directory, replace `darwin-x86_64` with the matching
folder on your machine.

Android emulator notes:

- `fission-shell-winit` forces `WGPU_BACKEND=gl` on Android when `WGPU_BACKEND` is unset so the emulator avoids the unstable Vulkan/SwiftShader path
- set `WGPU_BACKEND=vulkan` explicitly only if you want to audit that backend on a real device
- when `FISSION_TEST_CONTROL_PORT` is set, the Android shell keeps the event loop polling so semantic test-control commands stay responsive through `adb forward`

## Current scope

- `MobileApp` wrapper for the shared `fission-shell-winit` runtime
- Android `android_main` entry support
- iOS simulator app-bundle packaging through `examples/mobile-smoke/platforms/ios/`
- Android emulator packaging/launcher scripts through `examples/mobile-smoke/platforms/android/`
- host-side screenshot/test-control transport via `FISSION_TEST_CONTROL_PORT`
- smoke coverage through `examples/mobile-smoke/`

## Next work

- iOS device packaging/signing beyond the simulator path
- touch and gesture input mapping to Fission `InputEvent` types
- safe-area insets and display-cutout awareness
- soft keyboard / IME handling

More setup detail lives in `../../../docs/platform-smoke-tests.md`.
