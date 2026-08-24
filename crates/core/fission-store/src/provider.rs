use crate::{
    SqlError, SqlExecuteResult, SqlMigrationResult, SqlMigrations, SqlQuery, SqlRows, SqlStatement,
    SqlTransaction, SqlTransactionResult, StoreBatch, StoreBatchResult, StoreContains, StoreEntry,
    StoreError, StoreGet, StoreListPrefix, StoreRemove, StoreSet, StoreValue,
};
use std::future::Future;
use std::pin::Pin;

/// Future returned by a store provider.
///
/// Browser providers run on the single-threaded WebAssembly executor, while
/// native providers may move their work onto a shell worker thread.
#[cfg(target_arch = "wasm32")]
pub type StoreFuture<T> = Pin<Box<dyn Future<Output = T> + 'static>>;

/// Future returned by a store provider.
#[cfg(not(target_arch = "wasm32"))]
pub type StoreFuture<T> = Pin<Box<dyn Future<Output = T> + Send + 'static>>;

/// Provider for Fission's typed, namespaced key/value store.
///
/// Implementing this trait does not claim SQL support. Shell builders which
/// require SQL accept [`SqlStoreProvider`] instead.
pub trait StoreProvider: 'static {
    fn get(&self, request: StoreGet) -> StoreFuture<Result<Option<StoreValue>, StoreError>>;
    fn contains(&self, request: StoreContains) -> StoreFuture<Result<bool, StoreError>>;
    fn set(&self, request: StoreSet) -> StoreFuture<Result<(), StoreError>>;
    fn remove(&self, request: StoreRemove) -> StoreFuture<Result<bool, StoreError>>;
    fn batch(&self, request: StoreBatch) -> StoreFuture<Result<StoreBatchResult, StoreError>>;
    fn list_prefix(
        &self,
        request: StoreListPrefix,
    ) -> StoreFuture<Result<Vec<StoreEntry>, StoreError>>;
}

/// Store provider which guarantees SQLite-compatible SQL execution.
///
/// This is a compile-time capability boundary. A provider implementing only
/// [`StoreProvider`] cannot be installed where an application requires SQL.
pub trait SqlStoreProvider: StoreProvider {
    fn execute(&self, statement: SqlStatement) -> StoreFuture<Result<SqlExecuteResult, SqlError>>;

    fn query(&self, request: SqlQuery) -> StoreFuture<Result<SqlRows, SqlError>>;

    fn transaction(
        &self,
        transaction: SqlTransaction,
    ) -> StoreFuture<Result<SqlTransactionResult, SqlError>>;

    fn migrate(
        &self,
        migrations: SqlMigrations,
    ) -> StoreFuture<Result<SqlMigrationResult, SqlError>>;
}
