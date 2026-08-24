//! SQLite implementations of Fission's storage contracts.

#[cfg(all(feature = "native", not(target_arch = "wasm32")))]
mod native;
#[cfg(all(feature = "native", not(target_arch = "wasm32")))]
pub use native::SqliteStore;

#[cfg(all(feature = "web", target_arch = "wasm32"))]
mod web;
#[cfg(all(feature = "web", target_arch = "wasm32"))]
pub use web::WebSqliteStore;

pub const FISSION_STORE_SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS fission (
    scope TEXT NOT NULL,
    owner TEXT NOT NULL DEFAULT '',
    namespace TEXT NOT NULL,
    key TEXT NOT NULL,
    value BLOB NOT NULL,
    revision INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (scope, owner, namespace, key)
) WITHOUT ROWID;
"#;

/// Official SQLite WASM module bundled by Fission's Web scaffolder.
pub const SQLITE_WEB_MODULE: &[u8] = include_bytes!("../assets/web/sqlite3.mjs");
/// Official SQLite WASM binary bundled by Fission's Web scaffolder.
pub const SQLITE_WEB_WASM: &[u8] = include_bytes!("../assets/web/sqlite3.wasm");
/// Fission's main-thread request bridge for the SQLite worker.
pub const SQLITE_WEB_BRIDGE: &[u8] = include_bytes!("../assets/web/fission-sqlite.mjs");
/// Fission's SQLite worker implementation.
pub const SQLITE_WEB_WORKER: &[u8] = include_bytes!("../assets/web/fission-sqlite-worker.mjs");
/// Attribution for the vendored official SQLite WebAssembly distribution.
pub const SQLITE_WEB_NOTICE: &[u8] = include_bytes!("../assets/web/NOTICE.txt");
