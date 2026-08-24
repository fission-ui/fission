//! Typed effect capabilities for persistent Fission stores.

use crate::{CapabilityType, OperationCapability};
use fission_store::{
    SqlError, SqlExecuteResult, SqlQuery, SqlRows, SqlStatement, SqlTransaction,
    SqlTransactionResult, StoreBatch, StoreBatchResult, StoreEntry, StoreError, StoreGet,
    StoreListPrefix, StoreRemove, StoreSet, StoreValue,
};

pub struct StoreGetCapability;
impl OperationCapability for StoreGetCapability {
    type Request = StoreGet;
    type Ok = Option<StoreValue>;
    type Err = StoreError;
}

pub struct StoreSetCapability;
impl OperationCapability for StoreSetCapability {
    type Request = StoreSet;
    type Ok = ();
    type Err = StoreError;
}

pub struct StoreRemoveCapability;
impl OperationCapability for StoreRemoveCapability {
    type Request = StoreRemove;
    type Ok = bool;
    type Err = StoreError;
}

pub struct StoreBatchCapability;
impl OperationCapability for StoreBatchCapability {
    type Request = StoreBatch;
    type Ok = StoreBatchResult;
    type Err = StoreError;
}

pub struct StoreListPrefixCapability;
impl OperationCapability for StoreListPrefixCapability {
    type Request = StoreListPrefix;
    type Ok = Vec<StoreEntry>;
    type Err = StoreError;
}

pub const STORE_GET: CapabilityType<StoreGetCapability> = CapabilityType::new("fission.store.get");
pub const STORE_SET: CapabilityType<StoreSetCapability> = CapabilityType::new("fission.store.set");
pub const STORE_REMOVE: CapabilityType<StoreRemoveCapability> =
    CapabilityType::new("fission.store.remove");
pub const STORE_BATCH: CapabilityType<StoreBatchCapability> =
    CapabilityType::new("fission.store.batch");
pub const STORE_LIST_PREFIX: CapabilityType<StoreListPrefixCapability> =
    CapabilityType::new("fission.store.list_prefix");

#[cfg(feature = "store-sql")]
pub struct SqlExecuteCapability;
#[cfg(feature = "store-sql")]
impl OperationCapability for SqlExecuteCapability {
    type Request = SqlStatement;
    type Ok = SqlExecuteResult;
    type Err = SqlError;
}

#[cfg(feature = "store-sql")]
pub struct SqlQueryCapability;
#[cfg(feature = "store-sql")]
impl OperationCapability for SqlQueryCapability {
    type Request = SqlQuery;
    type Ok = SqlRows;
    type Err = SqlError;
}

#[cfg(feature = "store-sql")]
pub struct SqlTransactionCapability;
#[cfg(feature = "store-sql")]
impl OperationCapability for SqlTransactionCapability {
    type Request = SqlTransaction;
    type Ok = SqlTransactionResult;
    type Err = SqlError;
}

#[cfg(feature = "store-sql")]
pub const SQL_EXECUTE: CapabilityType<SqlExecuteCapability> =
    CapabilityType::new("fission.store.sql.execute");
#[cfg(feature = "store-sql")]
pub const SQL_QUERY: CapabilityType<SqlQueryCapability> =
    CapabilityType::new("fission.store.sql.query");
#[cfg(feature = "store-sql")]
pub const SQL_TRANSACTION: CapabilityType<SqlTransactionCapability> =
    CapabilityType::new("fission.store.sql.transaction");
