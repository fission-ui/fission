# Pokémon Card Store

A server-side Fission example that sells collectible Pokémon cards using normal Fission widgets rendered to HTML by the server shell.

It demonstrates:

- revalidated routes with a five-minute TTL and tag invalidation metadata;
- one route per product card, including cacheable detail pages under `/cards/<slug>`;
- the default Moka-backed cache used through the server shell's `Cache` trait;
- server-side `FutureBuilder` job draining before HTML is returned;
- signed HTTP action dispatch into normal reducers through real HTML forms;
- add-to-basket buttons that post signed action tokens back to the server route;
- route-local progressive-enhancement worker declarations loaded from generated WASM artifacts;
- route-local WASM island declarations loaded from generated WASM artifacts;
- generated per-worker and per-island WASM shim crates copied into `/assets/...`;
- production-style Rust organisation with data, server setup, and reusable widget components split into modules.

Run it locally:

```sh
fission server serve --project-dir examples/pokemon-card-store
```

Then open `http://127.0.0.1:8124/`.

Check server rendering and generate browser artifacts through the Fission CLI:

```sh
fission server check --project-dir examples/pokemon-card-store
fission server artifacts --project-dir examples/pokemon-card-store
```

Useful files:

- `src/server.rs` wires the server route, revalidation policy, worker, and island.
- `src/app.rs` builds the page from Fission widgets and registers the reducers used by jobs and signed actions.
- `src/components/` contains the reusable page sections.
- `src/data.rs` defines the store data and a sample job spec for catalog loading.
- `src/workers.rs` and `src/islands.rs` provide the browser artifact entry points.
