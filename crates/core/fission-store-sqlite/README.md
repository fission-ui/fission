# fission-store-sqlite

SQLite providers for Fission's portable value-store and SQL contracts. It supplies a bundled native provider and the official SQLite WASM/OPFS integration used by browser applications.

Application developers should normally use the [`fission`](https://crates.io/crates/fission) facade rather than depend on this crate directly. Enable `store-sqlite-native` for desktop, mobile, Terminal or SSR, and `store-sqlite-web` for Web. The recommended setup is:

```sh
fission add-capability storage --project-dir .
```

The CLI installs and validates the browser worker, bridge, SQLite WASM module and OPFS proxy as well as the matching Cargo features.

## Providers

- `SqliteStore` uses bundled SQLite through `rusqlite` on native targets. `FISSION_STORE_PATH` can override the application-data database path.
- `WebSqliteStore` runs the official SQLite WASM build in a worker and persists through the `opfs-wl` VFS. The database is origin-scoped and namespaced by the Fission application ID.

Web storage requires cross-origin isolation headers:

```text
Cross-Origin-Opener-Policy: same-origin
Cross-Origin-Embedder-Policy: require-corp
```

Separate tabs use independent SQLite connections coordinated through Web Locks. Keep migrations bounded and close pages running an older storage build during a one-time backend migration.

The provider implements both `StoreProvider` and `SqlStoreProvider`, so typed key/value operations, parameterised SQL, migrations and transactions use one database authority. Static-site builds have no runtime storage; SSR storage is server-side.

See the [storage and SQLite guide](https://fission.rs/docs/guides/storage-and-sqlite/) and the [`store-sqlite` example](https://github.com/fission-ui/fission/tree/main/examples/store-sqlite).

## Licence

Apache-2.0.
