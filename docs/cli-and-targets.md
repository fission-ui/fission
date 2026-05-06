# Fission CLI and target status

## Commands

Create a new app:

```sh
fission init my-app
```

Create a new app against a local Fission checkout:

```sh
fission init my-app --local-path /path/to/fission
```

Add more platform targets:

```sh
cargo fission add-target web ios android --project-dir my-app
```

The generated project contains:

- `src/main.rs` desktop entrypoint
- `src/app.rs` minimal counter app
- `fission.toml` target manifest
- `platforms/<target>/README.md` target notes and prerequisites

## Verified flow

This branch now has a verified scaffolding smoke test for the desktop path:

```sh
cargo run -p fission-cli --bin fission -- init /tmp/demo-app --local-path "$PWD"
cargo run -p fission-cli --bin cargo-fission -- fission add-target web ios android --project-dir /tmp/demo-app
cd /tmp/demo-app
cargo check
```

That flow completes successfully today.

The repository also has two verified mobile compile-smoke paths:

1. the checked-in `examples/mobile-smoke/` example
2. a CLI-generated app after `cargo fission add-target ios android web`

Direct example commands:

```sh
cargo check -p fission-shell-mobile -p mobile-smoke --target aarch64-apple-ios
```

On macOS, Android also works with the NDK toolchain environment configured:

```sh
export ANDROID_HOME="$HOME/Library/Android/sdk"
export ANDROID_NDK="$ANDROID_HOME/ndk/24.0.8215888"
export ANDROID_TOOLCHAIN="$ANDROID_NDK/toolchains/llvm/prebuilt/darwin-x86_64/bin"
export CC_aarch64_linux_android="$ANDROID_TOOLCHAIN/aarch64-linux-android24-clang"
export AR_aarch64_linux_android="$ANDROID_TOOLCHAIN/llvm-ar"

cargo check -p fission-shell-mobile -p mobile-smoke --target aarch64-linux-android
```

Generated-app commands from the scaffolded project root:

```sh
cargo check --target aarch64-apple-ios
# after exporting the Android env block from below
cargo check --target aarch64-linux-android
```

## Current target status

| Target | Scaffolded by CLI | Compile smoke in repo | Runnable today | Notes |
|---|---|---:|---:|---|
| Windows | yes | yes | yes | Uses the generated desktop entrypoint |
| macOS | yes | yes | yes | Uses the generated desktop entrypoint |
| Linux | yes | yes | yes | Uses the generated desktop entrypoint |
| iOS | yes | yes | no | the checked-in mobile smoke example and a CLI-generated app both cross-compile, but `fission add-target` does not generate an Xcode launcher yet |
| Android | yes | yes | no | the checked-in mobile smoke example and a CLI-generated app both cross-compile, but Android still needs NDK env vars and no Gradle/NativeActivity packaging is generated yet |
| Web | yes | no | no | `fission-shell-web` is still a placeholder; there is no runnable `web-smoke` example yet |

## Toolchains, env vars, and paths

Install the Rust targets:

```sh
rustup target add aarch64-apple-ios aarch64-linux-android wasm32-unknown-unknown
```

iOS prerequisites:

- Xcode installed
- `xcrun --sdk iphoneos --show-sdk-path` must resolve an iPhoneOS SDK
- smoke command:

```sh
cargo check -p fission-shell-mobile -p mobile-smoke --target aarch64-apple-ios
```

Android prerequisites:

- Android SDK installed
- Android NDK installed
- the verified macOS paths on this branch were:
  - `ANDROID_HOME=$HOME/Library/Android/sdk`
  - `ANDROID_NDK=$ANDROID_HOME/ndk/24.0.8215888`
  - toolchain bin dir: `$ANDROID_NDK/toolchains/llvm/prebuilt/darwin-x86_64/bin`
- required env:

```sh
export ANDROID_HOME="$HOME/Library/Android/sdk"
export ANDROID_NDK="$ANDROID_HOME/ndk/24.0.8215888"
export ANDROID_TOOLCHAIN="$ANDROID_NDK/toolchains/llvm/prebuilt/darwin-x86_64/bin"
export CC_aarch64_linux_android="$ANDROID_TOOLCHAIN/aarch64-linux-android24-clang"
export AR_aarch64_linux_android="$ANDROID_TOOLCHAIN/llvm-ar"
```

If your NDK uses a different host directory, replace `darwin-x86_64` with the matching prebuilt folder.

WASM prerequisites:

- `rustup target add wasm32-unknown-unknown`
- `cargo install wasm-pack`
- current status: documentation only; the checked-in web shell/runtime path is still under `crates/shell/fission-shell-web`

Relevant paths:

- mobile shell: `crates/shell/fission-shell-mobile/`
- web shell: `crates/shell/fission-shell-web/`
- mobile smoke example: `examples/mobile-smoke/`
- target scaffolding docs in generated apps: `platforms/<target>/README.md`

## Immediate next work

1. implement `fission-shell-web` with a WASM entrypoint and WebGPU surface
2. teach `fission add-target` to generate real iOS and Android launchers/package files
3. add a first-party `web-smoke` example once `fission-shell-web` exists
4. add first-party devtools hooks so the CLI can launch apps with widget-tree inspection enabled
