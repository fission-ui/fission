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

Read [`src/lib.rs`](src/lib.rs) first. It starts the shared Compose tree and
registers the document-reading job on the normal shell async host. The tap
counter and image remain basic input/rendering checks.

## Native document and print qualification

[`src/document_probe.rs`](src/document_probe.rs) adds four explicit checks using
Fission's real host providers, not mock replacements. Text import consumes the
returned one-shot stream in a job and accepts at most 64 KiB of UTF-8. Visible
pending, cancelled, successful and failed states distinguish an opened system
surface from a completed operation. English and Spanish messages are checked-in
`i18n/*.properties` files: one literal `key=value` per line, split on the first
equals sign, with no escaping or interpolation syntax.

For the iOS simulator, use the normal project entrypoint with an existing device
and your approved target/cache directory on the shared drive. Use the Fission
CLI built from the same revision as this example.

```bash
export CARGO_TARGET_DIR=/path/to/shared/build/mobile-smoke
export CARGO_HOME="$CARGO_TARGET_DIR/cargo-home"
export FISSION_TEST_CONTROL_PORT=48711
fission run --target ios --project-dir examples/mobile-smoke --device <simulator-udid>
```

Do not write fixtures directly into another application's sandbox. Save the
test document through its native picker, then use that actual file for import:

1. Choose **Save test note**, then cancel the native picker. Expect the cancelled
   status; any previous selected document and imported text remain unchanged.
2. Save again, choose a provider-backed destination and confirm. Expect a
   successful write of `fission-document-qa.txt`, not merely a selected filename.
3. Choose **Import a text document** and select that saved file. Expect its
   complete contents, including `Revision 1`, plus the byte count.
4. Choose **Write updated note** without picking a destination again. Import
   the same file again and verify `Revision 2`. This exercises the retained
   writable destination after its picker has closed.
5. Cancel the import picker. Expect the cancelled status and the previously
   imported text to remain. Repeat a successful import after cancellation to
   check that subsequent picker requests still work.
6. Choose **Print two-page PDF**. The checked-in 888-byte PDF has clearly
   numbered pages. Select only page 2 in the system sheet and verify the preview
   and actual output where printing is available. Cancel another print request
   and inspect the resulting status. Android's `Presented` outcome only confirms
   that the system print surface opened; it cannot prove printing finished.

The app's button semantic identifiers are `mobile-smoke.documents.save`,
`mobile-smoke.documents.import`, `mobile-smoke.documents.update` and
`mobile-smoke.documents.print`. Status and imported text use
`mobile-smoke.documents.status` and `mobile-smoke.documents.content`.
LiveTest can activate these Fission controls and inspect their results. The
native Files/print sheets are UIKit or Android UI, outside the Fission widget
tree: complete those steps manually or with platform UI automation. An app-only
LiveTest screenshot does not prove what the native sheet displayed. Capture the
whole simulator when recording native-sheet evidence:

```bash
xcrun simctl io <simulator-udid> screenshot /path/to/shared/qa/document-picker.png
```

The desktop preview can exercise import and printing with its native providers.
Path-based save/write requires an enabled filesystem provider; otherwise it
reports an explicit unsupported-capability error. Mobile save handles use the
native picked-file capability directly. No physical printing or provider-backed
device qualification is claimed by the fixture, source checks or unit tests.
