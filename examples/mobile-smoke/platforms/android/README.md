# Android target

Runnable emulator target. The CLI generates a Gradle Android project shell plus scripts that build, install, and launch the Fission app on an Android emulator.

- Install the Rust target: `rustup target add aarch64-linux-android`.
- Run `fission doctor android --project-dir .` to check SDK, NDK, emulator, and Rust target setup.
- Run `fission devices --project-dir .` to list connected Android devices and configured emulators.
- Run `fission run --target android --project-dir .` to build, install, launch, and attach to logs.
- Run `fission run --target android --device <adb-serial> --project-dir .` to launch on a specific device.
- Run `fission test --target android --project-dir .` for an emulator launch plus test-control health check.
- Run `./platforms/android/run-emulator.sh` from the project root to build, package, install, and launch the app on the configured emulator.
- Override `ANDROID_HOME`, `ANDROID_NDK`, `ANDROID_MIN_API_LEVEL`, `ANDROID_TARGET_API_LEVEL`, `ANDROID_AVD_NAME`, or `ANDROID_SYSTEM_IMAGE` if your local SDK setup differs.
- Set `ANDROID_EMULATOR_HEADLESS=1` for background/CI runs, or `ANDROID_EMULATOR_RESTART=1` to relaunch a hidden emulator visibly.
- The generated package uses `assets/app-icon.png` as its default launcher icon.
- Configure `[app.splash]` in `fission.toml` to generate the native Android launch theme, splash background, static image, and optional Android animated drawable.
- Run `fission add-capability nfc --project-dir .` to add NFC manifest permission and feature declarations.
- Run `fission add-capability notifications --project-dir .` to add Android notification permission for API 33 and newer.
- Run `fission add-capability biometric --project-dir .` to add biometric manifest permissions.
- Run `fission add-capability passkeys --project-dir .` to record passkey/WebAuthn use. Android passkeys also require Digital Asset Links and host Credential Manager integration for production sign-in.
- Run `fission add-capability bluetooth --project-dir .` to add Bluetooth permissions and optional hardware feature declarations.
- Run `fission add-capability barcode-scanner --project-dir .` to add camera permission for barcode scanning.
- Run `fission add-capability camera --project-dir .` to add camera permission and optional camera/flash hardware feature declarations.
- Run `fission add-capability geolocation --project-dir .` to add location permissions.
- Run `fission add-capability haptics --project-dir .` to add the vibration permission.
- Run `fission add-capability microphone --project-dir .` to add audio recording permission.
- Run `fission add-capability volume-control --project-dir .` to add Android audio settings permission.
- Run `fission add-capability wifi --project-dir .` to add Wi-Fi permissions and optional hardware feature declarations.
- Set `FISSION_TEST_CONTROL_PORT=<host-port>` before `run-emulator.sh`; the script forwards it to the fixed in-app device port.
