# Android target

Runnable emulator target. The CLI generates a NativeActivity manifest plus shell scripts that build, install, and launch the Fission app on an Android emulator.

- Install the Rust target: `rustup target add aarch64-linux-android`.
- Run `cargo fission doctor android --project-dir .` to check SDK, NDK, emulator, and Rust target setup.
- Run `cargo fission devices --project-dir .` to list connected Android devices and configured emulators.
- Run `cargo fission run --target android --project-dir .` to build, install, launch, and attach to logs.
- Run `cargo fission run --target android --device <adb-serial> --project-dir .` to launch on a specific device.
- Run `cargo fission test --target android --project-dir .` for an emulator launch plus test-control health check.
- Run `./platforms/android/run-emulator.sh` from the project root to build, package, install, and launch the app on the configured emulator.
- Override `ANDROID_HOME`, `ANDROID_NDK`, `ANDROID_MIN_API_LEVEL`, `ANDROID_TARGET_API_LEVEL`, `ANDROID_AVD_NAME`, or `ANDROID_SYSTEM_IMAGE` if your local SDK setup differs.
- Set `ANDROID_EMULATOR_HEADLESS=1` for background/CI runs, or `ANDROID_EMULATOR_RESTART=1` to relaunch a hidden emulator visibly.
- The generated package uses `assets/app-icon.png` as its default launcher icon.
- Set `FISSION_TEST_CONTROL_PORT=<host-port>` before `run-emulator.sh`; the script forwards it to the fixed in-app device port.
