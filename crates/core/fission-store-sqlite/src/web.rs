//! Browser SQLite provider backed by the official SQLite WASM OPFS VFS.

use fission_store::{
    SqlError, SqlErrorKind, SqlExecuteResult, SqlQuery, SqlRows, SqlStatement, SqlStoreProvider,
    SqlTransaction, SqlTransactionResult, StoreBatch, StoreBatchResult, StoreContains, StoreEntry,
    StoreError, StoreErrorKind, StoreFuture, StoreGet, StoreListPrefix, StoreProvider, StoreRemove,
    StoreSet, StoreValue,
};
use serde::{de::DeserializeOwned, Serialize};
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = globalThis, js_name = __fissionSqliteRequest)]
    fn sqlite_request(request: JsValue) -> js_sys::Promise;
}

/// Browser provider. The generated Web bootstrap installs its worker bridge
/// before starting the Fission application.
#[derive(Clone, Copy, Debug, Default)]
pub struct WebSqliteStore;

impl WebSqliteStore {
    pub fn new() -> Self {
        Self
    }
}

async fn request<Q, R>(operation: &str, request: Q) -> Result<R, String>
where
    Q: Serialize,
    R: DeserializeOwned,
{
    #[derive(Serialize)]
    struct Request<'a, Q> {
        operation: &'a str,
        request: Q,
    }

    let value = serde_wasm_bindgen::to_value(&Request { operation, request })
        .map_err(|error| error.to_string())?;
    let response = JsFuture::from(sqlite_request(value))
        .await
        .map_err(js_error_message)?;
    serde_wasm_bindgen::from_value(response).map_err(|error| error.to_string())
}

fn js_error_message(value: JsValue) -> String {
    value
        .dyn_ref::<js_sys::Error>()
        .map(|error| error.message().into())
        .or_else(|| value.as_string())
        .unwrap_or_else(|| "SQLite worker request failed".to_string())
}

fn store_error(message: String) -> StoreError {
    StoreError::new(StoreErrorKind::Backend, message)
}

fn sql_error(message: String) -> SqlError {
    SqlError::new(SqlErrorKind::Backend, message)
}

impl StoreProvider for WebSqliteStore {
    fn get(&self, request_value: StoreGet) -> StoreFuture<Result<Option<StoreValue>, StoreError>> {
        Box::pin(async move {
            request("store_get", request_value)
                .await
                .map_err(store_error)
        })
    }

    fn contains(&self, request_value: StoreContains) -> StoreFuture<Result<bool, StoreError>> {
        Box::pin(async move {
            request("store_contains", request_value)
                .await
                .map_err(store_error)
        })
    }

    fn set(&self, request_value: StoreSet) -> StoreFuture<Result<(), StoreError>> {
        Box::pin(async move {
            request("store_set", request_value)
                .await
                .map_err(store_error)
        })
    }

    fn remove(&self, request_value: StoreRemove) -> StoreFuture<Result<bool, StoreError>> {
        Box::pin(async move {
            request("store_remove", request_value)
                .await
                .map_err(store_error)
        })
    }

    fn batch(
        &self,
        request_value: StoreBatch,
    ) -> StoreFuture<Result<StoreBatchResult, StoreError>> {
        Box::pin(async move {
            request("store_batch", request_value)
                .await
                .map_err(store_error)
        })
    }

    fn list_prefix(
        &self,
        request_value: StoreListPrefix,
    ) -> StoreFuture<Result<Vec<StoreEntry>, StoreError>> {
        Box::pin(async move {
            request("store_list_prefix", request_value)
                .await
                .map_err(store_error)
        })
    }
}

impl SqlStoreProvider for WebSqliteStore {
    fn execute(&self, statement: SqlStatement) -> StoreFuture<Result<SqlExecuteResult, SqlError>> {
        Box::pin(async move { request("sql_execute", statement).await.map_err(sql_error) })
    }

    fn query(&self, query: SqlQuery) -> StoreFuture<Result<SqlRows, SqlError>> {
        Box::pin(async move { request("sql_query", query).await.map_err(sql_error) })
    }

    fn transaction(
        &self,
        transaction: SqlTransaction,
    ) -> StoreFuture<Result<SqlTransactionResult, SqlError>> {
        Box::pin(async move {
            request("sql_transaction", transaction)
                .await
                .map_err(sql_error)
        })
    }
}
