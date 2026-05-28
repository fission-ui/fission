# Mobile Smoke

Mobile Smoke is the smallest shared app used to verify the mobile shell path. It exists to prove that the same Fission state, reducers, and widget tree can run through `MobileApp` on Android and iOS while still offering a desktop preview.

Use this example when you are checking SDK setup, simulator/emulator configuration, or regressions in mobile input and rendering.

## Targets

The example supports:

- Desktop preview through `DesktopApp`.
- Android emulator/device through `MobileApp` and `android_main`.
- iOS simulator/device through `MobileApp`.

## Run it

Desktop preview:

```bash
cargo run -p mobile-smoke
```

iOS simulator:

```bash
rustup target add aarch64-apple-ios aarch64-apple-ios-sim
./examples/mobile-smoke/platforms/ios/run-sim.sh
./examples/mobile-smoke/platforms/ios/test-sim.sh
```

Android emulator:

```bash
rustup target add aarch64-linux-android
./examples/mobile-smoke/platforms/android/run-emulator.sh
./examples/mobile-smoke/platforms/android/test-emulator.sh
```

You can also use the CLI workflow:

```bash
fission run --target ios --project-dir examples/mobile-smoke
fission run --target android --project-dir examples/mobile-smoke
```

## What to look at

- [`src/lib.rs`](src/lib.rs) contains the shared app state, reducer, widget tree, mobile app construction, and Android entrypoint.
- [`src/main.rs`](src/main.rs) delegates to desktop or mobile entrypoints depending on the target.
- [`platforms/ios/run-sim.sh`](platforms/ios/run-sim.sh) and [`platforms/ios/test-sim.sh`](platforms/ios/test-sim.sh) show the simulator build, launch, and smoke-test flow.
- [`platforms/android/run-emulator.sh`](platforms/android/run-emulator.sh) and [`platforms/android/test-emulator.sh`](platforms/android/test-emulator.sh) show the emulator build, launch, and smoke-test flow.
- [`platforms/ios/Info.plist`](platforms/ios/Info.plist) and [`platforms/android/AndroidManifest.xml`](platforms/android/AndroidManifest.xml) show the mobile app metadata used by the generated bundles.

## Features exercised

- `MobileApp` startup for Android and iOS.
- `DesktopApp` preview for the same widget tree.
- Android `android_main` integration.
- iOS simulator packaging through generated platform scripts.
- Test-control port support for emulator/simulator smoke tests.

## Learning path

Read [`src/lib.rs`](src/lib.rs) first. The app is intentionally compact: `SmokeState` stores the tap count, `on_increment` updates it, and `MobileSmokeApp::build` renders the same UI for every supported platform.
