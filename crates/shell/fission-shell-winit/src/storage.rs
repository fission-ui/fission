use fission_shell::async_host::AsyncRegistry;
use fission_store::{SqlStoreProvider, StoreProvider};
use std::sync::Arc;

trait ShellStoreProvider: StoreProvider + Send + Sync {}
impl<T: StoreProvider + Send + Sync> ShellStoreProvider for T {}

trait ShellSqlStoreProvider: SqlStoreProvider + Send + Sync {}
impl<T: SqlStoreProvider + Send + Sync> ShellSqlStoreProvider for T {}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn register_store_provider<P>(registry: &mut AsyncRegistry, provider: Arc<P>)
where
    P: StoreProvider + Send + Sync,
{
    register_store_operations(registry, provider);
}

#[cfg(target_arch = "wasm32")]
pub(crate) fn register_store_provider<P>(registry: &mut AsyncRegistry, provider: Arc<P>)
where
    P: StoreProvider + Send + Sync,
{
    register_store_operations(registry, provider);
}

fn register_store_operations<P>(registry: &mut AsyncRegistry, provider: Arc<P>)
where
    P: ShellStoreProvider,
{
    let operation = provider.clone();
    registry.register_operation_capability(fission_core::STORE_GET, move |request, _| {
        operation.get(request)
    });
    let operation = provider.clone();
    registry.register_operation_capability(fission_core::STORE_SET, move |request, _| {
        operation.set(request)
    });
    let operation = provider.clone();
    registry.register_operation_capability(fission_core::STORE_CONTAINS, move |request, _| {
        operation.contains(request)
    });
    let operation = provider.clone();
    registry.register_operation_capability(fission_core::STORE_REMOVE, move |request, _| {
        operation.remove(request)
    });
    let operation = provider.clone();
    registry.register_operation_capability(fission_core::STORE_BATCH, move |request, _| {
        operation.batch(request)
    });
    registry.register_operation_capability(fission_core::STORE_LIST_PREFIX, move |request, _| {
        provider.list_prefix(request)
    });
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn register_sql_store_provider<P>(registry: &mut AsyncRegistry, provider: Arc<P>)
where
    P: SqlStoreProvider + Send + Sync,
{
    register_store_provider(registry, provider.clone());
    register_sql_operations(registry, provider);
}

#[cfg(target_arch = "wasm32")]
pub(crate) fn register_sql_store_provider<P>(registry: &mut AsyncRegistry, provider: Arc<P>)
where
    P: SqlStoreProvider + Send + Sync,
{
    register_store_provider(registry, provider.clone());
    register_sql_operations(registry, provider);
}

fn register_sql_operations<P>(registry: &mut AsyncRegistry, provider: Arc<P>)
where
    P: ShellSqlStoreProvider,
{
    let operation = provider.clone();
    registry.register_operation_capability(fission_core::SQL_EXECUTE, move |request, _| {
        operation.execute(request)
    });
    let operation = provider.clone();
    registry.register_operation_capability(fission_core::SQL_QUERY, move |request, _| {
        operation.query(request)
    });
    registry.register_operation_capability(fission_core::SQL_TRANSACTION, move |request, _| {
        provider.transaction(request)
    });
}

#[cfg(all(feature = "store-sqlite-native", not(target_arch = "wasm32")))]
pub(crate) fn register_default_native_store(
    registry: &mut AsyncRegistry,
    application_name: &str,
) -> anyhow::Result<()> {
    if registry.has_operation_capability(fission_core::SQL_QUERY) {
        return Ok(());
    }
    let path = default_store_path(application_name)?;
    let provider = fission_store_sqlite::SqliteStore::open(&path).map_err(|error| {
        anyhow::anyhow!("failed to open SQLite store {}: {error}", path.display())
    })?;
    register_sql_store_provider(registry, Arc::new(provider));
    Ok(())
}

#[cfg(all(feature = "store-sqlite-web", target_arch = "wasm32"))]
pub(crate) fn register_default_web_store(registry: &mut AsyncRegistry) {
    if registry.has_operation_capability(fission_core::SQL_QUERY) {
        return;
    }
    register_sql_store_provider(
        registry,
        Arc::new(fission_store_sqlite::WebSqliteStore::new()),
    );
}

#[cfg(all(feature = "store-sqlite-native", not(target_arch = "wasm32")))]
fn default_store_path(application_name: &str) -> anyhow::Result<std::path::PathBuf> {
    if let Some(path) = std::env::var_os("FISSION_STORE_PATH") {
        return Ok(path.into());
    }
    let root = default_data_directory().ok_or_else(|| {
        anyhow::anyhow!(
            "the operating system did not expose an application-data directory; set FISSION_STORE_PATH"
        )
    })?;
    let directory = root
        .join("fission")
        .join(safe_application_name(application_name));
    std::fs::create_dir_all(&directory).map_err(|error| {
        anyhow::anyhow!(
            "failed to create Fission store directory {}: {error}",
            directory.display()
        )
    })?;
    Ok(directory.join("store.sqlite3"))
}

#[cfg(all(feature = "store-sqlite-native", not(target_arch = "wasm32")))]
fn default_data_directory() -> Option<std::path::PathBuf> {
    #[cfg(target_os = "windows")]
    {
        std::env::var_os("LOCALAPPDATA")
            .or_else(|| std::env::var_os("APPDATA"))
            .map(Into::into)
    }
    #[cfg(target_os = "macos")]
    {
        std::env::var_os("HOME")
            .map(std::path::PathBuf::from)
            .map(|home| home.join("Library/Application Support"))
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        std::env::var_os("XDG_DATA_HOME")
            .map(std::path::PathBuf::from)
            .or_else(|| {
                std::env::var_os("HOME")
                    .map(std::path::PathBuf::from)
                    .map(|home| home.join(".local/share"))
            })
    }
}

#[cfg(all(feature = "store-sqlite-native", not(target_arch = "wasm32")))]
fn safe_application_name(value: &str) -> String {
    let safe = value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.') {
                character
            } else {
                '_'
            }
        })
        .collect::<String>();
    if safe.is_empty() {
        "app".to_string()
    } else {
        safe
    }
}

#[cfg(test)]
mod tests {
    #[test]
    #[cfg(all(feature = "store-sqlite-native", not(target_arch = "wasm32")))]
    fn application_names_are_safe_path_components() {
        assert_eq!(
            super::safe_application_name("My App / Test"),
            "My_App___Test"
        );
    }
}
