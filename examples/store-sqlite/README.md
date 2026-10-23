# SQLite store

This example explicitly enables Fission's desktop SQLite provider. CLI-managed
applications use `fission add-capability storage` instead. It applies an
ordered migration, performs a project write and an audit write in one
`SqlTransaction`, extends that transaction from a separate module, and queries
the resulting rows through reducer effects.

Run it from the repository root:

```bash
cargo run -p store-sqlite
```

The interface uses Compose while storage remains an asynchronous Fission effect:
the component renders current state and dispatches actions, synchronous reducers
request SQL work, and completion reducers apply typed results. No database work
runs during component conversion and the UI thread is never blocked waiting for
SQLite.

Projects are assigned durable database identifiers and the repeated result rows
use those identifiers as widget keys. This preserves retained identity if query
order changes and models the pattern production applications should follow.

For a CLI-created application, run `fission add-capability storage --project-dir
.` so the required native and Web assets are installed together. See the
[Storage and SQLite guide](https://fission.rs/docs/guides/storage-and-sqlite/) for
scopes, migrations, transactions and Web deployment requirements.

## Licence

Apache-2.0
