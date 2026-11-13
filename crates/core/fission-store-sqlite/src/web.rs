//! Browser SQLite provider backed by the official SQLite WASM OPFS VFS.

use fission_store::{
    SqlError, SqlErrorKind, SqlExecuteResult, SqlMigrationResult, SqlMigrations, SqlQuery, SqlRows,
    SqlStatement, SqlStoreProvider, SqlTransaction, SqlTransactionResult, StoreBatch,
    StoreBatchResult, StoreContains, StoreEntry, StoreError, StoreErrorKind, StoreFuture, StoreGet,
    StoreListPrefix, StoreProvider, StoreRemove, StoreSet, StoreValue,
};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;

const SQLITE_REQUEST_BRIDGE: &str = "__fissionSqliteRequest";
const SQLITE_CLOSE_DATABASE_BRIDGE: &str = "__fissionSqliteCloseDatabase";

// Resolve the host functions dynamically: wasm-bindgen's extern descriptors
// collide when an app links this crate from two distinct Git sources.

/// Browser provider. The generated Web bootstrap installs its worker bridge
/// before starting the Fission application.
#[derive(Clone, Debug, Default)]
pub struct WebSqliteStore {
    database_name: Option<String>,
}

impl WebSqliteStore {
    pub fn new() -> Self {
        Self {
            database_name: None,
        }
    }

    /// Opens a database isolated from the application's default database and
    /// from every other named database. Use a stable, application-owned name
    /// such as a workspace ID. The browser host hashes the name before using
    /// it to select an OPFS file, so names are never interpreted as paths.
    pub fn open_database(name: impl Into<String>) -> Result<Self, SqlError> {
        let name = name.into();
        if name.trim().is_empty() || name.len() > 1024 {
            return Err(SqlError::new(
                SqlErrorKind::InvalidParameter,
                "Web SQLite database names must contain 1 to 1024 UTF-8 bytes",
            ));
        }
        Ok(Self {
            database_name: Some(name),
        })
    }

    /// Closes this browser context's database connection and cancels its open
    /// transaction, including when another tab owns the actual SQLite worker.
    /// If this context owns the worker, closing it also releases its writer
    /// lock. A later request reopens the same OPFS database. Call this after
    /// pending operations complete.
    pub fn close(&self) -> Result<(), SqlError> {
        ensure_sqlite_bridge().map_err(sql_error)?;
        let database_name = self
            .database_name
            .as_deref()
            .map(JsValue::from_str)
            .unwrap_or(JsValue::UNDEFINED);
        call_sqlite_bridge(SQLITE_CLOSE_DATABASE_BRIDGE, &database_name)
            .map(|_| ())
            .map_err(sql_error)
    }

    /// Begins a worker-owned transaction which remains open across requests.
    /// Requests for this same database from other browser contexts receive
    /// `SqlErrorKind::Busy` until this transaction commits or rolls back.
    /// Cancelling this future requests rollback of its eventual transaction,
    /// even if the worker has begun it before Rust receives the handle.
    pub async fn begin_transaction(
        &self,
        options: WebSqliteTransactionOptions,
    ) -> Result<WebSqliteTransaction, SqlError> {
        let promise = start_request(self.database_name.as_deref(), "sql_tx_begin", options)
            .map_err(sql_error)?;
        let rollback = TransactionBeginRollback {
            database_name: self.database_name.clone(),
            promise: Some(promise.clone()),
        };
        let result: TransactionIdResponse = await_response(promise).await.map_err(sql_error)?;
        rollback.disarm();
        Ok(WebSqliteTransaction {
            id: result.transaction_id,
            database_name: self.database_name.clone(),
            closed: false,
        })
    }
}

/// Per-transaction SQLite configuration. These values are serializable so the
/// worker, rather than the page, owns the database connection and callbacks.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WebSqliteTransactionOptions {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub authorizer_policy: Option<WebSqliteAuthorizerPolicy>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub operation_context: Option<WebSqliteOperationContext>,
}

/// One ordered operation in a worker-side job on an open Web SQLite transaction.
///
/// A job executes all of its steps in one worker message. Query results from
/// earlier steps are returned to Rust in order; SQL remains parameterized and
/// subject to the transaction's active authorizer policy.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum WebSqliteJobStep {
    Execute(SqlStatement),
    Query(SqlStatement),
}

/// Result of one [`WebSqliteJobStep`] in the same position in the job result.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum WebSqliteJobResult {
    Execute(SqlExecuteResult),
    Query(SqlRows),
}

/// Rule-based SQLite authorizer policy installed for one transaction.
/// Rules are evaluated in order; the first matching rule wins.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WebSqliteAuthorizerPolicy {
    pub default_decision: WebSqliteAuthorizerDecision,
    #[serde(default)]
    pub rules: Vec<WebSqliteAuthorizerRule>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum WebSqliteAuthorizerDecision {
    Allow,
    Deny,
}

/// SQLite authorizer operations. `Read`, `Write`, `Attach`, and `Admin` group
/// the corresponding SQLite action codes; the remaining variants target one
/// exact SQLite action. `Read` allows reads, selections and function calls,
/// but not PRAGMAs: those can modify persistent state and require an explicit
/// `Pragma` rule. Custom SQL functions must enforce their own side-effect policy.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum WebSqliteAuthorizerAction {
    Any,
    Read,
    Write,
    Attach,
    Admin,
    CreateIndex,
    CreateTable,
    CreateTempIndex,
    CreateTempTable,
    CreateTempTrigger,
    CreateTempView,
    CreateTrigger,
    CreateView,
    Delete,
    DropIndex,
    DropTable,
    DropTempIndex,
    DropTempTable,
    DropTempTrigger,
    DropTempView,
    DropTrigger,
    DropView,
    Insert,
    Pragma,
    Select,
    Transaction,
    Update,
    Detach,
    AlterTable,
    Reindex,
    Analyze,
    CreateVtable,
    DropVtable,
    Function,
    Savepoint,
    Recursive,
}

/// Match an authorizer callback and allow or deny that operation. Exact
/// argument filters are case-insensitive. Prefix filters support dynamic
/// protected table names such as `_internal_`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WebSqliteAuthorizerRule {
    pub action: WebSqliteAuthorizerAction,
    pub decision: WebSqliteAuthorizerDecision,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub first_argument: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub first_argument_prefix: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub first_argument_suffix: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub second_argument: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub second_argument_prefix: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub second_argument_suffix: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub database: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub database_not: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trigger: Option<String>,
}

/// Values exposed by the transaction-context SQL functions.
/// `pdb_next_op_seq()` returns this value first, then increments on each call.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WebSqliteOperationContext {
    pub operation_id: String,
    pub next_op_seq: i64,
}

/// An opaque session that captures changes only from the transaction's
/// explicitly attached tables.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WebSqliteSession {
    id: String,
}

impl WebSqliteSession {
    pub fn id(&self) -> &str {
        &self.id
    }
}

/// Conflict resolution used while applying a SQLite Session changeset.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum WebSqliteChangesetConflictPolicy {
    Omit,
    Replace,
    #[default]
    Abort,
}

/// A transaction handle bound by the worker to the browser tab that created it.
pub struct WebSqliteTransaction {
    id: String,
    database_name: Option<String>,
    closed: bool,
}

impl WebSqliteTransaction {
    pub fn id(&self) -> &str {
        &self.id
    }

    fn ensure_open(&self) -> Result<(), SqlError> {
        if self.closed {
            Err(SqlError::new(
                SqlErrorKind::Transaction,
                "Web SQLite transaction is closed",
            ))
        } else {
            Ok(())
        }
    }

    pub async fn execute(&mut self, statement: SqlStatement) -> Result<SqlExecuteResult, SqlError> {
        self.ensure_open()?;
        request(
            self.database_name.as_deref(),
            "sql_tx_execute",
            TransactionStatementRequest::new(&self.id, statement),
        )
        .await
        .map_err(sql_error)
    }

    pub async fn query(&mut self, statement: SqlStatement) -> Result<SqlRows, SqlError> {
        self.ensure_open()?;
        request(
            self.database_name.as_deref(),
            "sql_tx_query",
            TransactionStatementRequest::new(&self.id, statement),
        )
        .await
        .map_err(sql_error)
    }

    /// Execute an ordered group of SQL statements in the worker without
    /// crossing the browser-to-worker boundary for each statement. The batch
    /// runs inside this transaction, so its operation context and authorizer
    /// remain active and no other transaction can interleave. On success the
    /// transaction stays open; on a step error the worker rolls it back and
    /// invalidates this handle. Dropping the in-flight Rust future also queues
    /// a rollback after the synchronous worker job has completed.
    pub async fn run_job(
        &mut self,
        steps: Vec<WebSqliteJobStep>,
    ) -> Result<Vec<WebSqliteJobResult>, SqlError> {
        self.ensure_open()?;
        let database_name = self.database_name.clone();
        let transaction_id = self.id.clone();
        let request_value = TransactionJobRequest {
            transaction_id: transaction_id.clone(),
            steps,
        };
        let rollback =
            TransactionJobRollback::new(&mut self.closed, database_name.clone(), transaction_id);
        let results = request(database_name.as_deref(), "sql_tx_job", request_value)
            .await
            .map_err(sql_error)?;
        rollback.disarm();
        Ok(results)
    }

    pub async fn create_session(
        &mut self,
        tables: impl IntoIterator<Item = impl Into<String>>,
    ) -> Result<WebSqliteSession, SqlError> {
        self.ensure_open()?;
        let result: SessionIdResponse = request(
            self.database_name.as_deref(),
            "sql_tx_session_create",
            CreateSessionRequest {
                transaction_id: self.id.clone(),
                tables: tables.into_iter().map(Into::into).collect(),
            },
        )
        .await
        .map_err(sql_error)?;
        Ok(WebSqliteSession {
            id: result.session_id,
        })
    }

    pub async fn capture_changeset(
        &mut self,
        session: &WebSqliteSession,
    ) -> Result<Vec<u8>, SqlError> {
        self.ensure_open()?;
        let result: ChangesetResponse = request(
            self.database_name.as_deref(),
            "sql_tx_session_capture",
            SessionRequest {
                transaction_id: self.id.clone(),
                session_id: session.id.clone(),
            },
        )
        .await
        .map_err(sql_error)?;
        Ok(result.changeset)
    }

    pub async fn apply_changeset(
        &mut self,
        changeset: &[u8],
        conflict_policy: WebSqliteChangesetConflictPolicy,
    ) -> Result<(), SqlError> {
        self.ensure_open()?;
        request::<_, AppliedResponse>(
            self.database_name.as_deref(),
            "sql_tx_changeset_apply",
            ApplyChangesetRequest {
                transaction_id: self.id.clone(),
                changeset: changeset.to_vec(),
                conflict_policy,
            },
        )
        .await
        .map(|_| ())
        .map_err(sql_error)
    }

    pub async fn commit(&mut self) -> Result<(), SqlError> {
        self.ensure_open()?;
        request::<_, ()>(
            self.database_name.as_deref(),
            "sql_tx_commit",
            TransactionIdRequest::new(&self.id),
        )
        .await
        .map_err(sql_error)?;
        self.closed = true;
        Ok(())
    }

    pub async fn rollback(&mut self) -> Result<(), SqlError> {
        self.ensure_open()?;
        request::<_, ()>(
            self.database_name.as_deref(),
            "sql_tx_rollback",
            TransactionIdRequest::new(&self.id),
        )
        .await
        .map_err(sql_error)?;
        self.closed = true;
        Ok(())
    }
}

#[cfg(target_arch = "wasm32")]
struct TransactionJobRollback<'a> {
    closed: &'a mut bool,
    database_name: Option<String>,
    transaction_id: String,
    armed: bool,
}

#[cfg(target_arch = "wasm32")]
impl<'a> TransactionJobRollback<'a> {
    fn new(closed: &'a mut bool, database_name: Option<String>, transaction_id: String) -> Self {
        Self {
            closed,
            database_name,
            transaction_id,
            armed: true,
        }
    }

    fn disarm(mut self) {
        self.armed = false;
    }
}

#[cfg(target_arch = "wasm32")]
impl Drop for TransactionJobRollback<'_> {
    fn drop(&mut self) {
        if !self.armed {
            return;
        }
        *self.closed = true;
        let database_name = self.database_name.clone();
        let transaction_id = self.transaction_id.clone();
        wasm_bindgen_futures::spawn_local(async move {
            let _ = request::<_, ()>(
                database_name.as_deref(),
                "sql_tx_rollback",
                TransactionIdRequest::new(&transaction_id),
            )
            .await;
        });
    }
}

impl Drop for WebSqliteTransaction {
    fn drop(&mut self) {
        if self.closed {
            return;
        }
        let id = self.id.clone();
        let database_name = self.database_name.clone();
        wasm_bindgen_futures::spawn_local(async move {
            let _ = request::<_, ()>(
                database_name.as_deref(),
                "sql_tx_rollback",
                TransactionIdRequest::new(&id),
            )
            .await;
        });
    }
}

/// A BEGIN request can outlive its Rust future. Retain the started JS promise
/// until either the returned handle takes ownership or cancellation queues a
/// cleanup task which waits for its exact transaction ID before rolling back.
struct TransactionBeginRollback {
    database_name: Option<String>,
    promise: Option<js_sys::Promise>,
}

impl TransactionBeginRollback {
    fn disarm(mut self) {
        self.promise = None;
    }
}

impl Drop for TransactionBeginRollback {
    fn drop(&mut self) {
        let Some(promise) = self.promise.take() else {
            return;
        };
        let database_name = self.database_name.clone();
        wasm_bindgen_futures::spawn_local(async move {
            if let Ok(result) = await_response::<TransactionIdResponse>(promise).await {
                let _ = request::<_, ()>(
                    database_name.as_deref(),
                    "sql_tx_rollback",
                    TransactionIdRequest::new(&result.transaction_id),
                )
                .await;
            }
        });
    }
}

#[derive(Serialize)]
struct TransactionIdRequest {
    transaction_id: String,
}

impl TransactionIdRequest {
    fn new(id: &str) -> Self {
        Self {
            transaction_id: id.to_string(),
        }
    }
}

#[derive(Deserialize)]
struct TransactionIdResponse {
    transaction_id: String,
}

#[derive(Serialize)]
struct TransactionStatementRequest {
    transaction_id: String,
    statement: SqlStatement,
}

#[derive(Serialize)]
struct TransactionJobRequest {
    transaction_id: String,
    steps: Vec<WebSqliteJobStep>,
}

impl TransactionStatementRequest {
    fn new(id: &str, statement: SqlStatement) -> Self {
        Self {
            transaction_id: id.to_string(),
            statement,
        }
    }
}

#[derive(Serialize)]
struct CreateSessionRequest {
    transaction_id: String,
    tables: Vec<String>,
}

#[derive(Deserialize)]
struct SessionIdResponse {
    session_id: String,
}

#[derive(Serialize)]
struct SessionRequest {
    transaction_id: String,
    session_id: String,
}

#[derive(Deserialize)]
struct ChangesetResponse {
    changeset: Vec<u8>,
}

#[derive(Serialize)]
struct ApplyChangesetRequest {
    transaction_id: String,
    changeset: Vec<u8>,
    conflict_policy: WebSqliteChangesetConflictPolicy,
}

#[derive(Deserialize)]
struct AppliedResponse {
    #[serde(rename = "applied")]
    _applied: bool,
}

#[derive(Debug)]
struct WebSqliteError {
    code: Option<String>,
    sqlite_code: Option<i32>,
    message: String,
}

impl WebSqliteError {
    fn new(code: &str, message: impl Into<String>) -> Self {
        Self {
            code: Some(code.to_string()),
            sqlite_code: None,
            message: message.into(),
        }
    }
}

async fn request<Q, R>(
    database_name: Option<&str>,
    operation: &str,
    request: Q,
) -> Result<R, WebSqliteError>
where
    Q: Serialize,
    R: DeserializeOwned,
{
    await_response(start_request(database_name, operation, request)?).await
}

fn start_request<Q: Serialize>(
    database_name: Option<&str>,
    operation: &str,
    request: Q,
) -> Result<js_sys::Promise, WebSqliteError> {
    ensure_sqlite_bridge()?;
    #[derive(Serialize)]
    struct Request<'a, Q> {
        #[serde(rename = "databaseName", skip_serializing_if = "Option::is_none")]
        database_name: Option<&'a str>,
        operation: &'a str,
        request: Q,
    }

    let value = serde_wasm_bindgen::to_value(&Request {
        database_name,
        operation,
        request,
    })
    .map_err(|error| WebSqliteError::new("serialization", error.to_string()))?;
    call_sqlite_bridge(SQLITE_REQUEST_BRIDGE, &value)?
        .dyn_into::<js_sys::Promise>()
        .map_err(|_| {
            WebSqliteError::new(
                "invalid_bridge",
                "Fission Web SQLite request bridge did not return a Promise",
            )
        })
}

async fn await_response<R: DeserializeOwned>(
    promise: js_sys::Promise,
) -> Result<R, WebSqliteError> {
    let response = JsFuture::from(promise).await.map_err(js_error)?;
    serde_wasm_bindgen::from_value(response)
        .map_err(|error| WebSqliteError::new("serialization", error.to_string()))
}

fn ensure_sqlite_bridge() -> Result<(), WebSqliteError> {
    sqlite_bridge_function(SQLITE_REQUEST_BRIDGE)?;
    sqlite_bridge_function(SQLITE_CLOSE_DATABASE_BRIDGE)?;
    Ok(())
}

fn sqlite_bridge_function(name: &str) -> Result<js_sys::Function, WebSqliteError> {
    let value =
        js_sys::Reflect::get(&js_sys::global(), &JsValue::from_str(name)).map_err(js_error)?;
    value
        .dyn_into::<js_sys::Function>()
        .map_err(|_| WebSqliteError::new("unavailable", crate::MISSING_WEB_SQLITE_BRIDGE_ERROR))
}

fn call_sqlite_bridge(name: &str, argument: &JsValue) -> Result<JsValue, WebSqliteError> {
    sqlite_bridge_function(name)?
        .call1(&js_sys::global(), argument)
        .map_err(js_error)
}

fn js_error(value: JsValue) -> WebSqliteError {
    let message = value
        .dyn_ref::<js_sys::Error>()
        .map(|error| error.message().into())
        .or_else(|| value.as_string())
        .unwrap_or_else(|| "SQLite worker request failed".to_string());
    let code = js_sys::Reflect::get(&value, &JsValue::from_str("code"))
        .ok()
        .and_then(|value| value.as_string());
    let sqlite_code = js_sys::Reflect::get(&value, &JsValue::from_str("sqliteCode"))
        .ok()
        .and_then(|value| value.as_f64())
        .map(|value| value as i32);
    WebSqliteError {
        code,
        sqlite_code,
        message,
    }
}

fn store_error(error: WebSqliteError) -> StoreError {
    let kind = match error.code.as_deref() {
        Some("busy") => StoreErrorKind::Busy,
        Some("quota_exceeded") => StoreErrorKind::QuotaExceeded,
        Some("unavailable") | Some("transaction_lost") => StoreErrorKind::Unavailable,
        Some("read_only") => StoreErrorKind::ReadOnly,
        Some("serialization") => StoreErrorKind::Serialization,
        Some("invalid_request") => StoreErrorKind::InvalidRequest,
        _ => StoreErrorKind::Backend,
    };
    StoreError::new(kind, error.message)
}

fn sql_error(error: WebSqliteError) -> SqlError {
    let sqlite_primary = error.sqlite_code.map(|code| code & 0xff);
    let kind = match error.code.as_deref() {
        Some("busy") => SqlErrorKind::Busy,
        Some("transaction_lost") | Some("transaction_owner_mismatch") => SqlErrorKind::Transaction,
        Some("unavailable") => SqlErrorKind::Unavailable,
        Some("quota_exceeded") => SqlErrorKind::QuotaExceeded,
        Some("read_only") => SqlErrorKind::ReadOnly,
        _ => match sqlite_primary {
            Some(5 | 6) => SqlErrorKind::Busy,
            Some(19) => SqlErrorKind::Constraint,
            Some(8) => SqlErrorKind::ReadOnly,
            Some(23) => SqlErrorKind::ReadOnly,
            Some(1) => SqlErrorKind::InvalidSql,
            Some(13) => SqlErrorKind::QuotaExceeded,
            Some(10 | 14) => SqlErrorKind::Unavailable,
            _ => SqlErrorKind::Backend,
        },
    };
    let mut sql_error = SqlError::new(kind, error.message);
    if let Some(code) = error.sqlite_code {
        sql_error = sql_error.with_sqlite_code(code);
    }
    sql_error
}

impl StoreProvider for WebSqliteStore {
    fn get(&self, request_value: StoreGet) -> StoreFuture<Result<Option<StoreValue>, StoreError>> {
        let database_name = self.database_name.clone();
        Box::pin(async move {
            request(database_name.as_deref(), "store_get", request_value)
                .await
                .map_err(store_error)
        })
    }

    fn contains(&self, request_value: StoreContains) -> StoreFuture<Result<bool, StoreError>> {
        let database_name = self.database_name.clone();
        Box::pin(async move {
            request(database_name.as_deref(), "store_contains", request_value)
                .await
                .map_err(store_error)
        })
    }

    fn set(&self, request_value: StoreSet) -> StoreFuture<Result<(), StoreError>> {
        let database_name = self.database_name.clone();
        Box::pin(async move {
            request(database_name.as_deref(), "store_set", request_value)
                .await
                .map_err(store_error)
        })
    }

    fn remove(&self, request_value: StoreRemove) -> StoreFuture<Result<bool, StoreError>> {
        let database_name = self.database_name.clone();
        Box::pin(async move {
            request(database_name.as_deref(), "store_remove", request_value)
                .await
                .map_err(store_error)
        })
    }

    fn batch(
        &self,
        request_value: StoreBatch,
    ) -> StoreFuture<Result<StoreBatchResult, StoreError>> {
        let database_name = self.database_name.clone();
        Box::pin(async move {
            request(database_name.as_deref(), "store_batch", request_value)
                .await
                .map_err(store_error)
        })
    }

    fn list_prefix(
        &self,
        request_value: StoreListPrefix,
    ) -> StoreFuture<Result<Vec<StoreEntry>, StoreError>> {
        let database_name = self.database_name.clone();
        Box::pin(async move {
            request(database_name.as_deref(), "store_list_prefix", request_value)
                .await
                .map_err(store_error)
        })
    }
}

impl SqlStoreProvider for WebSqliteStore {
    fn execute(&self, statement: SqlStatement) -> StoreFuture<Result<SqlExecuteResult, SqlError>> {
        let database_name = self.database_name.clone();
        Box::pin(async move {
            request(database_name.as_deref(), "sql_execute", statement)
                .await
                .map_err(sql_error)
        })
    }

    fn query(&self, query: SqlQuery) -> StoreFuture<Result<SqlRows, SqlError>> {
        let database_name = self.database_name.clone();
        Box::pin(async move {
            request(database_name.as_deref(), "sql_query", query)
                .await
                .map_err(sql_error)
        })
    }

    fn transaction(
        &self,
        transaction: SqlTransaction,
    ) -> StoreFuture<Result<SqlTransactionResult, SqlError>> {
        let database_name = self.database_name.clone();
        Box::pin(async move {
            request(database_name.as_deref(), "sql_transaction", transaction)
                .await
                .map_err(sql_error)
        })
    }

    fn migrate(
        &self,
        migrations: SqlMigrations,
    ) -> StoreFuture<Result<SqlMigrationResult, SqlError>> {
        let database_name = self.database_name.clone();
        Box::pin(async move {
            request(database_name.as_deref(), "sql_migrate", migrations)
                .await
                .map_err(sql_error)
        })
    }
}
