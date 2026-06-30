# fission-shell-server

Server-side web shell for Fission applications.

This crate renders normal Fission widget trees to HTTP responses while keeping
server work aligned with Fission concepts: routes, jobs, actions/reducers,
services, cache policies, progressive browser workers, and focused WebAssembly
islands. It is the shell used by `fission server` and by server-rendered Fission
apps that need static pages, revalidated pages, private session pages, and small
browser-side enhancements without switching to a separate web component model.

Most applications should depend on the public `fission` facade with the
`server` feature instead of depending on this crate directly:

```toml
[dependencies]
fission = { version = "0.6.0", features = ["server"] }
```

Enable adapter features through the facade when you want to host the renderer
inside an existing HTTP stack:

```toml
[dependencies]
fission = { version = "0.6.0", features = ["server", "server-axum"] }
```

See the guides and reference at <https://fission.rs> for the server app model,
security controls, `fission.toml` options, browser islands, cache pipelines, and
Docker packaging workflow.
