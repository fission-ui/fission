# iOS target

Simulator target. The CLI generates a simulator app bundle template plus shell scripts that build, install, launch, and smoke-test the Fission app with `simctl`.

- Install the Rust targets: `rustup target add aarch64-apple-ios aarch64-apple-ios-sim`.
- Run `cargo fission doctor ios --project-dir .` to check Xcode, simulator, and Rust target setup.
- Confirm the simulator SDK path with `xcrun --sdk iphonesimulator --show-sdk-path`.
- Run `cargo fission devices --project-dir .` to list available iOS simulators.
- Run `cargo fission run --target ios --project-dir .` to build, install, launch, and attach to simulator logs.
- Run `cargo fission run --target ios --device <simulator-udid> --project-dir .` to launch on a specific simulator.
- Run `cargo fission test --target ios --project-dir .` for a simulator launch plus test-control health check.
- Run `./platforms/ios/run-sim.sh` from the project root to build, install, and launch the app on the first available iPhone simulator.
- The generated bundle uses `assets/app-icon.png` as its default app icon.
- Set `FISSION_TEST_CONTROL_PORT=<port>` before `run-sim.sh` to expose the in-app test control server on the host.
- Set `IOS_SIM_DEVICE_ID=<udid>` if you want a specific simulator device.
- Set `IOS_SIM_HEADLESS=1` for CI or background-only simulator runs; otherwise the script opens Simulator visibly.
