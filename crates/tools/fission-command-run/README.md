# fission-command-run

Run, build, test, logs, devices, and doctor workflows for the `fission` command.

`fission-command-run` implements the everyday development commands behind the public `fission` executable.

## What it contains

- `fission doctor` checks for Rust targets, SDKs, emulators, simulators, browsers, and host tools.
- `fission devices` detects runnable macOS, Windows, Linux, Web, Android, iOS, Terminal, Static site, SSR, and other configured targets where the command has a host workflow.
- `fission run` builds, launches, and attaches to app output where the platform supports it.
- `fission build`, `fission run`, and `fission test` for Web resolve the exact
  qualified CanvasKit bundle, verify its locked archive and payload digests, and
  stage the complete artifact under `platforms/web/canvaskit` before building
  the application Wasm.
- `fission run --target web` prints the renderer selected by the browser runtime,
  including CanvasKit's Skia software path when browser GPU rendering is not selected.
- `fission build`, `fission test`, and `fission logs` shared execution helpers.

## Design notes

The command should feel like one lifecycle tool, not separate platform scripts. Platform details are hidden where possible, but diagnostics stay explicit so developers can fix local setup quickly.

## Documentation

See the CLI reference at [fission.rs](https://fission.rs/docs/reference/cli/overview/).

## License

MIT
