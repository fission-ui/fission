# Web Smoke

Web Smoke is the smallest cross-platform example configured by `fission.toml`. It verifies that one Fission app can run on desktop, web, Android, and iOS from the same widget tree.

Use this example when you want a fast target/toolchain check before debugging a larger app.

## Targets

The project is configured for:

- Android
- iOS
- Linux
- macOS
- Web
- Windows

## Run it

Desktop:

```bash
fission run --project-dir examples/web-smoke
```

Web:

```bash
fission run --target web --project-dir examples/web-smoke
```

iOS simulator:

```bash
fission run --target ios --project-dir examples/web-smoke
```

Android emulator:

```bash
fission run --target android --project-dir examples/web-smoke
```

## What to look at

- [`fission.toml`](fission.toml) lists the enabled targets and app id.
- [`src/app.rs`](src/app.rs) contains the shared counter state, reducer, and widget tree.
- [`src/lib.rs`](src/lib.rs) provides the desktop, mobile, Android, and web entrypoints.
- [`src/main.rs`](src/main.rs) delegates to the correct entrypoint for each target.
- [`platforms/web/`](platforms/web) contains the generated web host page and helper scripts.
- [`platforms/ios/`](platforms/ios) and [`platforms/android/`](platforms/android) contain generated mobile packaging, run, and smoke-test scripts.

## Features exercised

- Fission CLI project configuration through `fission.toml`.
- Desktop, web, Android, and iOS shell startup from the same app code.
- WASM entrypoint through `wasm-bindgen`.
- Android `android_main` integration.
- Generated platform scripts for local runs and smoke tests.

## Learning path

Start with [`src/app.rs`](src/app.rs); it is just a counter. Then compare [`src/lib.rs`](src/lib.rs) and [`src/main.rs`](src/main.rs) to see how that same app is mounted into each target shell.
