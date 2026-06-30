# iOS target

Simulator target. The CLI generates a simulator app bundle template plus shell scripts that build, install, launch, and smoke-test the Fission app with `simctl`.

- Install the Rust targets: `rustup target add aarch64-apple-ios aarch64-apple-ios-sim`.
- Run `fission doctor ios --project-dir .` to check Xcode, simulator, and Rust target setup.
- Confirm the simulator SDK path with `xcrun --sdk iphonesimulator --show-sdk-path`.
- Run `fission devices --project-dir .` to list available iOS simulators.
- Run `fission run --target ios --project-dir .` to build, install, launch, and attach to simulator logs.
- Run `fission run --target ios --device <simulator-udid> --project-dir .` to launch on a specific simulator.
- Run `fission test --target ios --project-dir .` for a simulator launch plus test-control health check.
- Run `./platforms/ios/run-sim.sh` from the project root to build, install, and launch the app on the first available iPhone simulator.
- The generated bundle uses `assets/app-icon.png` as its default app icon.
- Configure `[app.splash]` in `fission.toml` to generate the native iOS launch storyboard and splash image copied into the simulator bundle.
- Run `fission add-capability nfc --project-dir .` to add the NFC usage description and entitlements file.
- Run `fission add-capability notifications --project-dir .` to record local-notification use. iOS prompts at runtime and does not require an Info.plist usage key for local notifications.
- Run `fission add-capability biometric --project-dir .` to add the Face ID usage description.
- Run `fission add-capability passkeys --project-dir .` to record passkey/WebAuthn use. iOS production passkeys require associated domains such as `webcredentials:example.com` in the app entitlements.
- Run `fission add-capability bluetooth --project-dir .` to add the Bluetooth usage description.
- Run `fission add-capability barcode-scanner --project-dir .` to add the camera usage description for barcode scanning.
- Run `fission add-capability camera --project-dir .` to add the camera usage description.
- Run `fission add-capability geolocation --project-dir .` to add the location usage description.
- Run `fission add-capability microphone --project-dir .` to add the microphone usage description.
- Run `fission add-capability wifi --project-dir .` to add Wi-Fi entitlements and the location usage description required by current-network information APIs.
- Volume control does not require an iOS Info.plist key in the generated scaffold.
- Haptics do not require an iOS Info.plist key in the generated scaffold.
- Set `FISSION_TEST_CONTROL_PORT=<port>` before `run-sim.sh` to expose the in-app test control server on the host.
- Set `IOS_SIM_DEVICE_ID=<udid>` if you want a specific simulator device.
- Set `IOS_SIM_HEADLESS=1` for CI or background-only simulator runs; otherwise the script opens Simulator visibly.
