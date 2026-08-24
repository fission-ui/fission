//! Portable storage contracts shared by Fission shells and providers.
//!
//! [`StoreProvider`] supplies the framework-owned key/value surface. Providers
//! that also implement [`SqlStoreProvider`] expose SQLite-compatible SQL without
//! replacing or bypassing that portable surface.

mod provider;
mod sql;
mod store;

pub use provider::{SqlStoreProvider, StoreFuture, StoreProvider};
pub use sql::{
    FromSqlValue, SqlColumn, SqlError, SqlErrorKind, SqlExecuteResult, SqlMigration, SqlMigrations,
    SqlParameters, SqlQuery, SqlRow, SqlRows, SqlStatement, SqlStepResult, SqlTransaction,
    SqlTransactionResult, SqlTransactionStep, SqlValue,
};
pub use store::{
    StoreAddress, StoreBatch, StoreBatchOperation, StoreBatchResult, StoreEntry, StoreError,
    StoreErrorKind, StoreGet, StoreKey, StoreListPrefix, StoreRemove, StoreScope, StoreSet,
    StoreValue,
};
