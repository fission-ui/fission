# fission-store-sqlite

SQLite providers for Fission's portable value-store and SQL contracts. It supplies a bundled native provider and the official SQLite WASM/OPFS integration used by browser applications.

Application developers should normally use the [`fission`](https://crates.io/crates/fission) facade rather than depend on this crate directly. Enable `store-sqlite-native` for desktop, mobile, Terminal or SSR, and `store-sqlite-web` for Web. The recommended setup is:

```sh
fission add-capability storage --project-dir .
```

The CLI installs and validates the browser worker, bridge, SQLite WASM module and OPFS proxy as well as the matching Cargo features.

## Providers

- `SqliteStore` uses bundled SQLite through `rusqlite` on native targets. `FISSION_STORE_PATH` can override the application-data database path.
- `WebSqliteStore` runs the official SQLite WASM build in a worker and persists through the `opfs-wl` VFS. The default database is origin-scoped and namespaced by the Fission application ID.

Web storage requires cross-origin isolation headers:

```text
Cross-Origin-Opener-Policy: same-origin
Cross-Origin-Embedder-Policy: require-corp
```

Browser contexts coordinate through Web Locks and share one owner worker's
SQLite connection for each application/database. Keep migrations bounded and
close pages running an older storage build during a one-time backend migration.

The provider implements both `StoreProvider` and `SqlStoreProvider`, so typed key/value operations, parameterised SQL, migrations and transactions use one database authority. Static-site builds have no runtime storage; SSR storage is server-side.

### Isolated Web databases

Use a stable name when an application owns several independent databases, such
as one database per workspace. Each name selects its own OPFS file,
worker, and cross-tab writer lock. The name is hashed by the browser host before
it is used to construct a filename. The no-argument constructor continues to
open the application's existing default database.

```rust,ignore
let workspace_store = WebSqliteStore::open_database(workspace_database_id)?;
workspace_store.migrate(workspace_migrations).await?;
workspace_store.close()?; // A later request reopens the same named database.
```

Use the same name to reopen that workspace database in a later session. Do not
use a temporary file path or a user-facing label as the key: renaming either
would select a different database. Named databases do not import or migrate
the application's legacy default `store.sqlite3` file.
Call `close()` after pending work completes. It disconnects this context and
requests rollback of its transaction, including when another tab owns the
worker. If this context owns the worker, it also terminates that worker and
releases the writer lock. Subsequent requests reopen the same OPFS database;
a borrower may reuse another context's existing worker. Remote rollback is
asynchronous, so competing requests may briefly return `Busy` whilst the owner
processes the disconnect.

### Worker-owned Web transactions

For browser runtimes that need to keep one SQLite transaction open across
several asynchronous steps, `WebSqliteStore::begin_transaction` returns an
opaque handle bound to the browser tab that created it. Other tabs receive a
typed `Busy` error while that handle is open. A dropped handle requests a
rollback; call `commit` or `rollback` explicitly when possible. Cancelling the
`begin_transaction` future also requests rollback of its eventual handle: a
BEGIN that reached the worker does not become an ownerless transaction merely
because Rust stopped awaiting its response.

Each browser context holds a lifetime Web Lock before dispatching requests. The
worker owner watches borrowers' lifetime locks and rolls back their transactions
when those contexts disappear, including abrupt frame destruction without an
unload handler. It does not expire healthy transactions on a heartbeat timer or
assume that a throttled background tab has died. Ordinary live transactions
should still be committed or rolled back promptly.

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
let results = transaction
    .run_job(vec![
        WebSqliteJobStep::Execute(insert_document),
        WebSqliteJobStep::Query(load_document),
    ])
    .await?;
let changeset = transaction.capture_changeset(&session).await?;
transaction.commit().await?;
```

`Read` authorizer permission covers SQLite reads, selections and function calls,
not PRAGMAs. SQLite uses the same PRAGMA action code for inspection and changes
such as `user_version` or `writable_schema`. Authorise an intentional PRAGMA
explicitly with a `Pragma` rule and appropriate argument filters. Custom SQL
functions must enforce their own side-effect policy; a function call is not
necessarily a pure read. These local connection rules do not replace server
authentication or authorisation.

SQLite Session tracking also inspects a captured table with `table_xinfo` when
its first change is recorded. A deny-by-default policy must explicitly allow
`Pragma` with `first_argument: "table_xinfo"`, `second_argument` equal to that
table name, and `database: "main"`. This read-only metadata exception does not
require granting other tables or modifying PRAGMAs. If denied, Session can retain
the authorisation error until changeset capture even when the INSERT succeeds.

`run_job` sends its ordered steps to the SQLite worker in one message and
returns one `WebSqliteJobResult` per step. Query results can be used by Rust
after the job completes; the transaction remains open for later session
capture, changeset application, or commit. A SQL or authorizer error rolls
back the transaction. If the Rust future is cancelled, Fission queues rollback
after the synchronous worker job finishes.

`pdb_current_operation_id()` and `pdb_next_op_seq()` are registered only while
the operation context is active. The worker invalidates the handle and rolls
back on statement, capture, or changeset-apply failure. A worker loss is
reported as `SqlErrorKind::Transaction` with a transaction-lost message; quota,
busy, unavailable-storage, and SQLite failures retain distinct error kinds.
Worker startup has bounded retries. If those fail, waiting and subsequent
requests return `Unavailable` instead of hanging; close and reopen that database
after correcting its hosting or storage prerequisite to retry. A failure to
acquire the required lifetime lock also fails closed.

## Verify the Web provider

Run the browser-transport regressions without compiling an application:

```sh
node --experimental-vm-modules --test \
  crates/core/fission-store-sqlite/tests/web/bridge-lifetime.test.mjs
```

These tests execute the actual bridge and worker modules with in-memory browser
transports and SQLite primitives. They cover close/disconnect ordering and
authorizer dispatch, not OPFS durability or the SQLite engine itself.

The real Chromium fixture also exercises Rust cancellation. From the workspace
root, with `CARGO_TARGET_DIR` set to the configured shared project target, build
and package its small Web-only test library:

```sh
cargo build -p fission-store-sqlite --features web \
  --example web_transaction_cancellation --target wasm32-unknown-unknown
wasm-bindgen \
  "$CARGO_TARGET_DIR/wasm32-unknown-unknown/debug/examples/web_transaction_cancellation.wasm" \
  --target web --out-dir crates/core/fission-store-sqlite/tests/web/pkg
```

Use the `wasm-bindgen` CLI matching the workspace's locked crate version. Serve
the crate directory with the COOP/COEP headers above, then point the browser
test at `/tests/web/index.html` on that server:

```sh
FISSION_SQLITE_WEB_TEST_URL=http://127.0.0.1:PORT/tests/web/index.html \
  cargo test -p fission-store-sqlite --test web_multitab_browser --features web
```

The test checks real SQLite authorizer rejection, transaction/session behaviour,
borrower close, abrupt borrower-frame removal, worker loss, and repeated drops
of a pending Rust BEGIN future. Its generated `tests/web/pkg` is ignored; no
second provider or simulated persistence is installed in the browser fixture.

See the [storage and SQLite guide](https://fission.rs/docs/guides/storage-and-sqlite/) and the [`store-sqlite` example](https://github.com/fission-ui/fission/tree/main/examples/store-sqlite).

## Licence

Apache-2.0.
