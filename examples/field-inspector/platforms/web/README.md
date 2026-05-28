# Web target

Runnable browser target. The CLI generates a WASM host page plus helper scripts that build the app with `wasm-pack` and serve it locally.

- Install the Rust target: `rustup target add wasm32-unknown-unknown`.
- Install `wasm-pack` once: `cargo install wasm-pack`.
- Install Node.js 22+ so the smoke test can inspect Chrome/Chromium CDP runtime and console output.
- Run `fission doctor web --project-dir .` to check wasm-pack, Node.js, Chrome/Chromium, and Rust target setup.
- Run `fission devices --project-dir .` to confirm Chrome/Chromium detection.
- Run `fission run --target web --project-dir .` to build, serve, open, and attach to the local server.
- Run `fission run --target web --detach --project-dir .` to keep the local server running in the background.
- Run `fission test --target web --project-dir .` for a headless Chrome/Chromium CDP smoke test.
- Run `./platforms/web/run-browser.sh` from the project root to build the wasm package and serve the app locally.
- Set `FISSION_WEB_PORT=<port>` or `FISSION_WEB_HOST=<host>` if the default `127.0.0.1:8123` does not suit your machine.
- Set `FISSION_WEB_OPEN=1` if you want the helper script to open a browser tab automatically.
- The generated page uses `assets/app-icon.png` as its default favicon/app icon seed.
