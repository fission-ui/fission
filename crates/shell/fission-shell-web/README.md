# fission-shell-web

Web shell for the Fission UI framework (WebAssembly target).

`fission-shell-web` is the current browser shell for running Fission applications via WebAssembly.
It wraps the shared `fission-shell-winit` runtime on the wasm target and appends the generated
canvas to the page automatically.

## Status

What is ready today:

- runnable `WebApp` wrapper backed by the shared winit runtime
- checked-in `examples/web-smoke/` browser example
- first-party `fission add-target web` launcher output

What is still missing:

- host-side browser test control equivalent to the desktop/mobile TCP server
- richer browser integration for clipboard, drag-and-drop, and IME edge cases

## WASM prerequisites

```sh
rustup target add wasm32-unknown-unknown
cargo install wasm-pack
```

Relevant paths:

- `crates/shell/fission-shell-web/`
- `examples/web-smoke/`

Do not treat `fission-shell-desktop` as the web entrypoint. The desktop shell carries
desktop-specific runtime and test-driver dependencies that are not the right long-term
WASM surface.

## Verified commands

Build and serve the checked-in example:

```sh
rustup target add wasm32-unknown-unknown
cargo install wasm-pack
./examples/web-smoke/platforms/web/run-browser.sh
```

Build a generated app after `cargo fission add-target web`:

```sh
./platforms/web/run-browser.sh
```

More setup detail lives in `../../../docs/platform-smoke-tests.md`.
