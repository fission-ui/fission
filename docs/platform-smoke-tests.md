# Platform smoke tests

This document records the current reproducible target setup for the Fission repository.

## Current status

| Target | Example / shell | Status | Notes |
|---|---|---|---|
| Desktop | `examples/mobile-smoke` + `fission-shell-mobile` | runnable | `cargo run -p mobile-smoke` uses the shared winit + Vello path on the host |
| iOS | `examples/mobile-smoke` + `fission-shell-mobile` | compile smoke verified | cross-target `cargo check` is green for both the checked-in example and a CLI-generated app |
| Android | `examples/mobile-smoke` + `fission-shell-mobile` | compile smoke verified | requires Android SDK + NDK env vars; both the checked-in example and a CLI-generated app cross-compile |
| Web/WASM | `crates/shell/fission-shell-web` | not runnable yet | toolchain setup is documented, but there is no checked-in web shell/runtime or `web-smoke` example yet |

## Rust targets

Install the Rust targets once:

```sh
rustup target add aarch64-apple-ios aarch64-linux-android wasm32-unknown-unknown
```

## iOS

Required tools:

- Xcode
- iPhoneOS SDK visible through `xcrun`

Sanity check:

```sh
xcrun --sdk iphoneos --show-sdk-path
```

Smoke command:

```sh
cargo check -p fission-shell-mobile -p mobile-smoke --target aarch64-apple-ios
```

Relevant paths:

- shell: `crates/shell/fission-shell-mobile/`
- example: `examples/mobile-smoke/`

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

Notes:

- the host prebuilt directory may be `darwin-arm64`, `darwin-x86_64`, or a Linux variant on other machines
- the smoke path only needs the toolchain env above; it does not currently need `cargo-apk` or `cargo-ndk`
- first-party Android packaging/launcher generation is not implemented yet

Smoke command:

```sh
cargo check -p fission-shell-mobile -p mobile-smoke --target aarch64-linux-android
```

Relevant paths:

- shell: `crates/shell/fission-shell-mobile/`
- example: `examples/mobile-smoke/`

## Web / WASM

Required tools today:

```sh
rustup target add wasm32-unknown-unknown
cargo install wasm-pack
```

Current state:

- `crates/shell/fission-shell-web/` exists, but is still a placeholder
- there is no checked-in `web-smoke` example yet
- `fission add-target web` only scaffolds the target and records it in `fission.toml`

This means the web path is not yet reproducibly runnable for third-party developers. The toolchain
instructions are here now so the eventual shell/example can land against a documented setup.

## Generated app smoke

A newly scaffolded app was also verified in this branch:

```sh
fission init /tmp/demo-app --local-path "$PWD"
cargo fission add-target ios android web --project-dir /tmp/demo-app
cd /tmp/demo-app
cargo check --target aarch64-apple-ios
# after exporting the Android env block from the Android section above
cargo check --target aarch64-linux-android
```

The generated app cross-compiles for iOS and Android after the target is added. It is still not runnable on those platforms because the CLI does not yet generate the native launcher/package files.
