# Field Inspector

Field Inspector is a realistic cross-platform Fission example for exercising built-in host capabilities inside a normal product workflow. A technician selects a work order, verifies the physical asset, captures evidence, reads nearby device context, unlocks protected notes, and submits a report.

By default the example runs in native host mode. Capability calls go to the active shell, and unsupported host APIs are shown as unavailable instead of being silently faked. Set `FISSION_FIELD_INSPECTOR_DEMO_HOSTS=1` when you want deterministic in-memory providers for screenshots, tests, or workflow demos on machines without the relevant hardware.

## Supported targets

The project is configured with `fission.toml` for:

- Android
- iOS
- Linux
- macOS
- Web
- Windows

Run `fission devices --project-dir examples/field-inspector` to see the devices available on your machine.

## Run it

Desktop preview:

```bash
cargo run -p field-inspector
```

Through the Fission CLI:

```bash
fission run --project-dir examples/field-inspector
fission run --target web --project-dir examples/field-inspector
fission run --target ios --project-dir examples/field-inspector
fission run --target android --project-dir examples/field-inspector
```

Deterministic demo mode:

```bash
FISSION_FIELD_INSPECTOR_DEMO_HOSTS=1 fission run --project-dir examples/field-inspector
FISSION_FIELD_INSPECTOR_DEMO_HOSTS=1 fission run --target web --project-dir examples/field-inspector
```

For platform prerequisites and environment variables, read the generated target notes:

- [`platforms/web/README.md`](platforms/web/README.md)
- [`platforms/ios/README.md`](platforms/ios/README.md)
- [`platforms/android/README.md`](platforms/android/README.md)
- [`platforms/macos/README.md`](platforms/macos/README.md)
- [`platforms/linux/README.md`](platforms/linux/README.md)
- [`platforms/windows/README.md`](platforms/windows/README.md)

## What to look at

- [`fission.toml`](fission.toml) declares the enabled targets and the host capabilities the app needs.
- [`src/lib.rs`](src/lib.rs) contains the shared desktop, mobile, Android, iOS, and web entrypoint wiring. It also chooses between native shell providers and deterministic demo providers.
- [`src/main.rs`](src/main.rs) is intentionally small and delegates to the platform-specific library entrypoints.
- [`src/model.rs`](src/model.rs) defines the app state, reducers, capability requests, success/error handling, deep-link handling, and notification-response handling.
- [`src/api.rs`](src/api.rs) defines the weather job and Open-Meteo request/response types used by `FutureBuilder`.
- [`src/data.rs`](src/data.rs) contains the seeded work orders and expected asset identifiers.
- [`src/components/app.rs`](src/components/app.rs) builds the shell layout, work-order rail, hero, and panel navigation.
- [`src/components/overview.rs`](src/components/overview.rs) shows the weather `FutureBuilder`, asset summary, metrics, and capability overview.
- [`src/components/panels.rs`](src/components/panels.rs) contains the Verify, Evidence, Sensors, Security, and Review panels.
- [`src/components/status.rs`](src/components/status.rs) renders capability readiness and the activity log.
- [`tests/live_e2e.rs`](tests/live_e2e.rs) launches the app with the live test driver and captures a screenshot.

## Features exercised

- Notifications, scheduled reminders, app badge updates, and notification responses.
- Deep links through the `field-inspector` scheme.
- Barcode and NFC asset verification.
- Camera photo capture and flashlight control.
- Microphone capture for voice evidence.
- Geolocation plus live weather enrichment.
- Bluetooth scans, connection, and characteristic reads.
- Wi-Fi availability and scans.
- Biometric unlock and passkey registration/authentication.
- Clipboard export, haptic feedback, and host volume adjustment.
- `FutureBuilder` for async data and normal reducers for local workflow state.

## Tests

Normal unit tests:

```bash
cargo test -p field-inspector
```

Live smoke test with screenshot capture:

```bash
cargo test -p field-inspector --test live_e2e field_inspector_runs_capability_workflow_smoke -- --ignored --nocapture
```

The live screenshot is written under `.artifacts/screenshots/examples/field-inspector/live_e2e/`.

## Learning path

Start in [`src/model.rs`](src/model.rs) with `StartInspection`. That reducer emits the initial capability checks and records activity-log rows. Then open [`src/components/panels.rs`](src/components/panels.rs) and follow each button to the reducer it triggers. Finally, inspect [`src/lib.rs`](src/lib.rs) to see how those abstract capability requests are connected to concrete shell hosts for every platform target.
