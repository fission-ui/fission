# fission-store

Portable storage contracts shared by Fission applications, shells and providers. The crate defines typed key/value operations and a SQLite-compatible query model without choosing a platform database implementation.

Application developers should normally enable storage through the [`fission`](https://crates.io/crates/fission) facade and the CLI:

```sh
fission add-capability storage --project-dir .
```

That command installs the correct Cargo features and, for Web, the required worker, bridge and SQLite WASM assets. Direct use of this crate is intended for provider authors and framework integration.

## Contracts

- `StoreKey<T>` combines a typed value with an intentional namespace, key and ownership scope.
- `StoreProvider` supports get, set, contains, remove, bounded prefix lists and atomic batches.
- `SqlStoreProvider` adds parameterised statements, queries, migrations and bounded transactions.
- `StoreScope` distinguishes application, session, user and named ownership rather than relying on ad-hoc key prefixes.
- Structured errors preserve whether a request was invalid, unavailable or failed in the provider.

Store and SQL work is asynchronous. A Fission reducer requests it through `ctx.effects.store()` or `ctx.effects.sql()` and handles completion through an explicit action. Do not block the UI thread or issue storage work during component conversion.

```rust
let key = StoreKey::<Preferences>::new("settings", "preferences");

ctx.effects
    .store()
    .get(key)
    .on_ok(ActionEnvelope::from(PreferencesLoaded))
    .on_err(ActionEnvelope::from(StorageFailed));
```

The default SQLite implementations live in `fission-store-sqlite`. Applications can install another provider through the shell configuration when their platform or deployment requires it.

See the [storage and SQLite guide](https://fission.rs/docs/guides/storage-and-sqlite/) and the [`store-sqlite` example](https://github.com/fission-ui/fission/tree/main/examples/store-sqlite).

## Licence

Apache-2.0.
