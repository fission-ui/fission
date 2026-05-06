# fission-shell-web

Web shell for the Fission UI framework (WebAssembly target).

`fission-shell-web` will provide the platform integration layer for running Fission applications
in the browser via WebAssembly. This includes DOM event translation, `requestAnimationFrame`
scheduling, WebGPU surface management, and clipboard / IME interop through the Web APIs.

## Status

This crate is still a placeholder.

What is ready today:

- CLI scaffolding for `web`
- documented wasm toolchain setup

What is not ready yet:

- runnable `fission-shell-web` runtime
- checked-in `web-smoke` example
- first-party `fission add-target web` launcher output

## WASM prerequisites

```sh
rustup target add wasm32-unknown-unknown
cargo install wasm-pack
```

The intended source path for web support is:

- `crates/shell/fission-shell-web/`

Do not treat `fission-shell-desktop` as the web entrypoint. The desktop shell carries
desktop-specific runtime and test-driver dependencies that are not the right long-term
WASM surface.

## Planned scope

- WASM entry point and `requestAnimationFrame` event loop.
- DOM event (pointer, keyboard, wheel, IME) translation to Fission `InputEvent`.
- WebGPU surface integration for the Vello rendering backend.
- Clipboard and drag-and-drop interop via the Web Clipboard and Drag APIs.
- Integration with `fission-shell` shared abstractions (`Platform`, `VideoBackend`).

More setup detail lives in `../../../docs/platform-smoke-tests.md`.
