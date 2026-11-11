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

### Worker-owned Web transactions

For browser runtimes that need to keep one SQLite transaction open across
several asynchronous steps, `WebSqliteStore::begin_transaction` returns an
opaque handle bound to the browser tab that created it. Other tabs receive a
typed `Busy` error while that handle is open. A dropped handle requests a
rollback; call `commit` or `rollback` explicitly when possible.

The Web-only API also exposes SQLite Session capture and changeset apply. A
session can observe only the tables passed to `create_session`, and an
authorizer policy plus transaction-scoped operation context can be installed
when the transaction begins. This lets a browser adapter keep database access
inside Fission's worker instead of opening another OPFS connection:

```rust,ignore
let mut transaction = store
    .begin_transaction(WebSqliteTransactionOptions {
        authorizer_policy: Some(policy),
        operation_context: Some(WebSqliteOperationContext {
            operation_id,
            next_op_seq: 1,
        }),
    })
    .await?;

let session = transaction.create_session(["documents", "document_pages"]).await?;
transaction.execute(insert_document).await?;
let changeset = transaction.capture_changeset(&session).await?;
transaction.commit().await?;
```

`pdb_current_operation_id()` and `pdb_next_op_seq()` are registered only while
the operation context is active. The worker invalidates the handle and rolls
back on statement, capture, or changeset-apply failure. A worker loss is
reported as `SqlErrorKind::Transaction` with a transaction-lost message; quota,
busy, unavailable-storage, and SQLite failures retain distinct error kinds.

See the [storage and SQLite guide](https://fission.rs/docs/guides/storage-and-sqlite/) and the [`store-sqlite` example](https://github.com/fission-ui/fission/tree/main/examples/store-sqlite).

## Licence

Apache-2.0.
