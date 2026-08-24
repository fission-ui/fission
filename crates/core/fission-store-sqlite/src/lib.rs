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
