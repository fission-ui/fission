# mobile-smoke

Shared mobile smoke example for the current `fission-shell-mobile` path.

## What it proves

- the shared runtime launches on the host through `MobileApp`
- the same example cross-compiles for iOS
- the same example cross-compiles for Android when the NDK toolchain env is set

## Commands

Desktop preview:

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
