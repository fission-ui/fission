# SSR target

Server-rendered Fission target. The CLI runs the app through the server shell for dynamic HTML, revalidated pages, server jobs, signed actions, worker artifacts, and focused browser islands.

- Configure `[server].entry` in `fission.toml` so the CLI can invoke the server app.
- Run `fission server check --project-dir .` to render all declared server routes.
- Run `fission server serve --project-dir .` to serve the app locally.
- Run `fission server artifacts --project-dir .` to generate browser worker and island WASM shims.
- Run `fission package --target ssr --format docker-image --release --project-dir .` to package the server app as an OCI/Docker image.
