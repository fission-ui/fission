# web-smoke

Browser smoke test for the Fission web shell.

## Desktop preview

```sh
cargo run -p web-smoke
```

## WASM / browser smoke

```sh
rustup target add wasm32-unknown-unknown
cargo install wasm-pack
./examples/web-smoke/platforms/web/run-browser.sh
./examples/web-smoke/platforms/web/test-browser.sh
```

The run script builds the wasm package and keeps serving the repository root at:

- `http://127.0.0.1:8123/examples/web-smoke/platforms/web/`

Set `FISSION_WEB_OPEN=1` if you want the script to open a browser tab after it starts the server.

The test script starts a transient server, launches Chrome/Chromium headlessly with a DevTools Protocol port, fails on runtime or console errors, and waits for a non-empty canvas. It stops the server when the test exits. Set `FISSION_CHROME=/path/to/chrome` if Chrome cannot be auto-detected.
