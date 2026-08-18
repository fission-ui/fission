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
- Chromium live testing through `fission test --target web` and `LiveTestClient`
- Fission-owned keyboard shortcuts, clipboard events, contextual actions, and
  IME composition
- deny-by-default browser behavior with explicit `BrowserDefaults` opt-ins

What is still missing:

- browser autocorrect/autofill and soft-keyboard replacement edge cases
- Firefox and WebKit live-test drivers

## Browser defaults

Fission suppresses browser behavior by default so application input is
consistent with native targets. Delegate only the categories your application
intentionally wants the browser to own:

```rust
WebApp::new(App)
    .with_browser_defaults(
        BrowserDefaults::CONTEXT_MENU | BrowserDefaults::WHEEL,
    )
    .run()
```

Categories not included in the allowlist remain Fission-owned. The default is
`BrowserDefaults::NONE`.

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

Build a generated app after `fission add-target web`:

```sh
./platforms/web/run-browser.sh
```

More setup detail lives in `../../../docs/platform-smoke-tests.md`.
