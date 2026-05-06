# fission-shell-mobile

Mobile shell for the Fission UI framework (iOS and Android).

`fission-shell-mobile` provides the current mobile bootstrap layer for running Fission
applications on iOS and Android. In this branch it is intentionally thin: it reuses the
shared winit + Vello desktop runtime path while the first dedicated mobile lifecycle and
packaging work is being built out.

## Status

Current branch status:

- host + iOS compile smoke: verified
- Android compile smoke: verified with the Android NDK toolchain env configured
- native packaging/launcher generation: not implemented yet
- touch, safe-area, soft-keyboard, and mobile-specific lifecycle hooks: still in progress

## Verified commands

Desktop preview of the shared UI path:

```sh
cargo run -p mobile-smoke
```

iOS compile smoke:

```sh
rustup target add aarch64-apple-ios
xcrun --sdk iphoneos --show-sdk-path
cargo check -p fission-shell-mobile -p mobile-smoke --target aarch64-apple-ios
```

Android compile smoke on macOS:

```sh
rustup target add aarch64-linux-android
export ANDROID_HOME="$HOME/Library/Android/sdk"
export ANDROID_NDK="$ANDROID_HOME/ndk/24.0.8215888"
export ANDROID_TOOLCHAIN="$ANDROID_NDK/toolchains/llvm/prebuilt/darwin-x86_64/bin"
export CC_aarch64_linux_android="$ANDROID_TOOLCHAIN/aarch64-linux-android24-clang"
export AR_aarch64_linux_android="$ANDROID_TOOLCHAIN/llvm-ar"

cargo check -p fission-shell-mobile -p mobile-smoke --target aarch64-linux-android
```

If your NDK uses a different host prebuilt directory, replace `darwin-x86_64` with the matching
folder on your machine.

## Current scope

- `MobileApp` wrapper for the shared runtime
- Android `android_main` entry support
- compile-smoke coverage through `examples/mobile-smoke/`

## Next work

- iOS shell using UIKit-specific lifecycle and surface management
- Android shell using a first-class `NativeActivity` / packaging story
- touch and gesture input mapping to Fission `InputEvent` types
- safe-area insets and display-cutout awareness
- soft keyboard / IME handling

More setup detail lives in `../../../docs/platform-smoke-tests.md`.
