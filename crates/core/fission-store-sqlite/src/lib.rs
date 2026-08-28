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
/// Official SQLite OPFS proxy used by the multi-tab `opfs-wl` VFS.
pub const SQLITE_WEB_OPFS_ASYNC_PROXY: &[u8] =
    include_bytes!("../assets/web/sqlite3-opfs-async-proxy.js");
/// Fission's main-thread request bridge for the SQLite worker.
pub const SQLITE_WEB_BRIDGE: &[u8] = include_bytes!("../assets/web/fission-sqlite.mjs");
/// Fission's SQLite worker implementation.
pub const SQLITE_WEB_WORKER: &[u8] = include_bytes!("../assets/web/fission-sqlite-worker.mjs");
/// Attribution for the vendored official SQLite WebAssembly distribution.
pub const SQLITE_WEB_NOTICE: &[u8] = include_bytes!("../assets/web/NOTICE.txt");

/// Diagnostic returned when Web SQLite Rust support is built without its host bridge.
pub const MISSING_WEB_SQLITE_BRIDGE_ERROR: &str = "Fission Web SQLite is enabled, but the browser host bridge is missing. Run `fission add-capability storage --project-dir .` and rebuild the Web application.";

#[cfg(test)]
mod tests {
    use super::{SQLITE_WEB_BRIDGE, SQLITE_WEB_OPFS_ASYNC_PROXY, SQLITE_WEB_WORKER};

    #[test]
    fn web_host_uses_multitab_vfs_and_application_namespace() {
        let bridge = std::str::from_utf8(SQLITE_WEB_BRIDGE).expect("bridge is UTF-8");
        let worker = std::str::from_utf8(SQLITE_WEB_WORKER).expect("worker is UTF-8");

        assert!(bridge.contains("__FISSION_SQLITE_BRIDGE__"));
        assert!(bridge.contains("fission_app_id"));
        assert!(worker.contains("OpfsWlDb"));
        assert!(worker.contains("/fission-apps/${namespace}/store.sqlite3"));
        assert!(worker.contains("navigator.locks.request"));
        assert!(worker.contains("navigator.storage.getDirectory"));
        assert!(!worker.contains("sqlite3.opfs.entryExists"));
        assert!(!SQLITE_WEB_OPFS_ASYNC_PROXY.is_empty());
    }
}
