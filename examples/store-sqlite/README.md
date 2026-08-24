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
