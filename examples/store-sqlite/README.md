# SQLite store

This example uses Fission's default desktop SQLite provider. It applies an
ordered migration, performs a project write and an audit write in one
`SqlTransaction`, extends that transaction from a separate module, and queries
the resulting rows through reducer effects.

Run it from the repository root:

```bash
cargo run -p store-sqlite
```
