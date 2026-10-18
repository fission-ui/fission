# Pokémon Card Store

A server-side Fission example that sells collectible Pokémon cards using normal Fission widgets rendered to HTML by the server shell.

It demonstrates:

- session-private server routes that can render shopper-specific basket state safely;
- one route per product card, including session-aware detail pages under `/cards/<slug>`;
- an in-memory cart service keyed by the Fission server session cookie;
- server-side `FutureBuilder` job draining before HTML is returned;
- signed HTTP action dispatch into normal reducers through real HTML forms;
- add-to-basket buttons that post signed action tokens back to the server route and persist the cart across page loads;
- route-local progressive-enhancement worker declarations loaded from generated WASM artifacts;
- route-local WASM island declarations loaded from generated WASM artifacts;
- the browser bridge ABI used by those artifacts to receive boot/event JSON and return constrained DOM operations;
- client-side island events that update semantic DOM targets without submitting a form or re-rendering the page;
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

- `src/server.rs` wires the server routes, session-private render policy, worker, and island.
- `src/app.rs` owns shared store state and exports the routed page components.
- `src/components/store_home_page.rs` and `store_card_page.rs` use Compose to build the two server-rendered routes and register the reducers used by jobs and signed actions.
- `src/cart.rs` contains the demo cart service used to retain basket state by session.
- `src/components/` contains the reusable page sections.
- `src/data.rs` defines the store data and a sample job spec for catalog loading.
- `src/workers.rs` and `src/islands.rs` provide the browser artifact entry points and bridge messages.
