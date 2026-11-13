# Web SQLite worker transactions and sessions

Status: implemented on `feature/docs2-goal-oriented-site`.

## Reported gap

A browser storage adapter needs to keep SQLite work on Fission's OPFS worker
while coordinating operation context, authorization, and committed
changesets. Before this change, `WebSqliteStore` exposed only single-request
transactions. The bridge retained the browser context for responses but did not
forward that identity to the worker, so it could not safely own transactions
across messages.

## Fission API

The Web storage capability installed by `fission add-capability storage
--project-dir .` supplies the generated worker, bridge, SQLite WASM module, and
OPFS proxy. A Web app enables the matching `store-sqlite-web` feature and uses
`WebSqliteStore` after Fission's generated Web bootstrap has installed the
bridge. No separate worker, OPFS connection, or SQL persistence path should be
created by the app.

The Web-only typed API is:

- `WebSqliteStore::begin_transaction(WebSqliteTransactionOptions)` returns a
  transaction handle owned by the originating browser tab.
- `WebSqliteTransaction::{execute, query, run_job, create_session,
  capture_changeset, apply_changeset, commit, rollback}` perform operations on
  the worker-owned SQLite connection. `run_job` executes ordered `Execute` and
  `Query` steps in one worker message while preserving the open transaction,
  operation context, and authorizer.
- `WebSqliteOperationContext` installs `pdb_current_operation_id()` and
  `pdb_next_op_seq()` only for the lifetime of that transaction.
- `WebSqliteAuthorizerPolicy` evaluates ordered allow/deny rules. It supports
  SQLite action groups (`Read`, `Write`, `Attach`, `Admin`), exact and
  prefix/suffix argument filters, and database exclusions.
- `WebSqliteChangesetConflictPolicy` selects `Abort`, `Omit`, or `Replace` for
  changeset conflicts.

Session capture is scoped to the explicit table list supplied to
`create_session`. Fission temporarily allows SQLite's own transaction control
while its Session extension captures or applies a changeset; caller SQL remains
under the configured authorizer and cannot take over the worker's transaction.

`Read` does not include `SQLITE_PRAGMA`: that code covers both reads and writes,
including persistent metadata changes. A caller needing a PRAGMA must supply
an explicit `Pragma` rule with suitable argument filters. The read group still
allows function calls; custom SQL functions must enforce their own side-effect
policy. This connection policy is not a replacement for server authorisation.

## Worker and error behavior

The bridge forwards a stable browser-context ID with each worker request. The
worker binds its opaque transaction ID to that context and serializes all
requests on the single OPFS connection. A concurrent transaction or a request
without the active transaction handle returns `Busy`. A different tab cannot
use the handle and receives `transaction_owner_mismatch` without aborting the
owner's work.

Statement, session-capture, changeset-apply, and commit failures roll back the
transaction and invalidate its ID. Commit and rollback invalidate the handle;
subsequent use returns `transaction_lost`. Dropping the Rust handle requests a
best-effort rollback. If the worker stops, the bridge reports
`transaction_lost` for transaction requests, restarts the worker, and SQLite
discards work that was not committed. Quota, unavailable storage, SQLite,
authorization, and busy errors remain distinguishable through the Rust error
types. A transaction job is synchronous on the worker and cannot interleave
with another request. It stops on the first failed step and rolls back the
transaction. Dropping its in-flight Rust future queues a rollback after the
worker finishes the current job.

Cancelling a pending `begin_transaction` future retains its started JavaScript
promise in a cleanup task. If BEGIN succeeds later, that task rolls back the
exact returned transaction ID; it cannot cancel an unrelated later transaction.
Once Rust returns the handle, the normal handle owns rollback instead.

Worker ownership and transaction-client lifetime are distinct. Each context
holds a per-database lifetime Web Lock before sending requests. The worker owner
watches borrower lifetime locks. Closing a borrower sends a disconnect message
and releases its lock; abrupt navigation/frame destruction releases the lock
through the browser even if no Rust destructor or unload callback runs. The
owner then tells the worker to roll back only that client's active transaction.
Queued requests precede disconnect on the same worker port. Watchers are aborted
when their owner closes and cannot roll back a different context's transaction.
No heartbeat lease expires a live background-tab transaction. Remote close is
asynchronous; callers can briefly receive `Busy` before disconnect is processed.
Startup retry exhaustion, synchronous Worker creation failure and lifetime-lock
acquisition failure reject waiting and subsequent requests as unavailable. The
unavailable owner answers discovery requests too, so a later borrower does not
wait indefinitely for a ready announcement which can never arrive. Close and
reopen the database after fixing the prerequisite to retry.

## Verification

The Chromium smoke test at
`crates/core/fission-store-sqlite/tests/web/index.html` checks:

- one tab cannot use another tab's transaction, start a competing transaction,
  or write without the active handle;
- denied SQL rolls back earlier writes and expires its handle;
- operation-context functions work only during their transaction and are
  removed after both rollback and commit;
- SQLite Session changes can be captured, applied, and committed;
- terminating the owner worker makes the transaction handle unusable and keeps
  its uncommitted write out of the database.
- a transaction job returns ordered execute/query results, retains its
  operation context and authorizer, and rolls back earlier steps when a later
  step is denied.
- closing a borrower or removing its frame releases its transaction without
  terminating another context's owner worker or committing the pending write;
- `Read` permission rejects a modifying PRAGMA and leaves `user_version`
  unchanged;
- dropping a pending Rust BEGIN future repeatedly does not strand the worker.

The fixture imports the Web-only `web_transaction_cancellation` Cargo example.
Build it for `wasm32-unknown-unknown` and package it with the matching
`wasm-bindgen` CLI into `crates/core/fission-store-sqlite/tests/web/pkg` first;
the crate README gives the exact commands.

The Node protocol suite executes the same bridge and worker modules, with
browser/SQLite primitives simulated in memory. It also checks unrelated-client
close isolation, close during an in-flight BEGIN and explicit PRAGMA permission:

```sh
node --experimental-vm-modules --test \
  crates/core/fission-store-sqlite/tests/web/bridge-lifetime.test.mjs
```

The browser test is run by setting `FISSION_SQLITE_WEB_TEST_URL` to the fixture
served with COOP/COEP headers and running:

```sh
cargo test -p fission-store-sqlite --test web_multitab_browser --features web
```

This is a SQLite host boundary, not an application database runtime. Operation
logs, schema policy, recovery, and business-level idempotency remain the
application's responsibility.
