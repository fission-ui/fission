# Duplicate Fission source link regression

This Wasm `cdylib` deliberately links the current path source of
`fission-store-sqlite` with the pre-fix Fission revision `c7075c0e`. Both
providers call the SQLite bridge, so both implementations must be retained in
the final module. Before the dynamic bridge lookup fix, wasm-bindgen generated
duplicate descriptor symbols for the two static JavaScript imports.

From the Fission workspace root, run:

```sh
scripts/test_store_sqlite_duplicate_wasm_sources.sh
```
