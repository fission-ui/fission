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
```

The script builds the wasm package and serves the repository root at:

- `http://127.0.0.1:8123/examples/web-smoke/platforms/web/`

Set `FISSION_WEB_OPEN=1` if you want the script to open a browser tab after it starts the server.
