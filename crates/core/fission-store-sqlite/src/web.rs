//! Browser SQLite provider backed by the official SQLite WASM OPFS VFS.

use std::cell::Cell;

use fission_store::{
    SqlError, SqlErrorKind, SqlExecuteResult, SqlMigrationResult, SqlMigrations, SqlQuery, SqlRows,
    SqlStatement, SqlStoreProvider, SqlTransaction, SqlTransactionResult, StoreBatch,
    StoreBatchResult, StoreContains, StoreEntry, StoreError, StoreErrorKind, StoreFuture, StoreGet,
    StoreListPrefix, StoreProvider, StoreRemove, StoreSet, StoreValue,
};
use serde::{
    de::{DeserializeOwned, SeqAccess, Visitor},
    ser::Serializer,
    Deserialize, Serialize,
};
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;

const SQLITE_REQUEST_BRIDGE: &str = "__fissionSqliteRequest";
const SQLITE_CLOSE_DATABASE_BRIDGE: &str = "__fissionSqliteCloseDatabase";

/// Largest chunk accepted by Web SQLite snapshot readers and import writers.
pub const MAX_WEB_SQLITE_STREAM_CHUNK_BYTES: usize = 1024 * 1024;

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

    /// Creates an immutable, consistent snapshot of this named database.
    ///
    /// The worker finishes a SQLite Online Backup into a private OPFS file
    /// before returning. It closes the source and snapshot database handles
    /// before callers transfer chunks to another provider. Query the snapshot
    /// before its first chunk read when metadata must match the exported image.
    pub async fn begin_snapshot(&self) -> Result<WebSqliteSnapshot, SqlError> {
        let begin_token = session_begin_token();
        let promise = start_request(
            self.database_name.as_deref(),
            "sqlite_snapshot_begin",
            SessionBeginRequest {
                begin_token: begin_token.clone(),
            },
        )
        .map_err(sql_error)?;
        let rollback = PendingSessionBeginRollback {
            database_name: self.database_name.clone(),
            cancel_operation: "sqlite_snapshot_cancel",
            pending_cancel_operation: "sqlite_session_cancel_pending",
            begin_token,
            promise: Some(promise.clone()),
        };
        let result: SessionBeginResponse = await_response(promise).await.map_err(sql_error)?;
        let byte_len = result
            .byte_len
            .as_deref()
            .and_then(|value| value.parse::<u64>().ok())
            .filter(|value| *value > 0)
            .ok_or_else(|| {
                SqlError::new(
                    SqlErrorKind::InvalidParameter,
                    "Web SQLite snapshot returned an invalid byte length",
                )
            })?;
        rollback.disarm();
        Ok(WebSqliteSnapshot {
            database_name: self.database_name.clone(),
            session_id: result.session_id,
            byte_len,
            reading_started: false,
            closed: false,
        })
    }

    /// Starts a bounded-memory import into a private OPFS staging file.
    ///
    /// Call [`WebSqliteImport::finish_stage`] after all chunks are written.
    /// That validates SQLite integrity and returns a read-only staged database;
    /// PersonalDB can inspect its identity and restore metadata before choosing
    /// either an atomic whole-database install or its own transactional restore.
    pub async fn begin_import(&self) -> Result<WebSqliteImport, SqlError> {
        let begin_token = session_begin_token();
        let promise = start_request(
            self.database_name.as_deref(),
            "sqlite_import_begin",
            SessionBeginRequest {
                begin_token: begin_token.clone(),
            },
        )
        .map_err(sql_error)?;
        let rollback = PendingSessionBeginRollback {
            database_name: self.database_name.clone(),
            cancel_operation: "sqlite_import_cancel",
            pending_cancel_operation: "sqlite_session_cancel_pending",
            begin_token,
            promise: Some(promise.clone()),
        };
        let result: SessionBeginResponse = await_response(promise).await.map_err(sql_error)?;
        rollback.disarm();
        Ok(WebSqliteImport {
            database_name: self.database_name.clone(),
            session_id: result.session_id,
            closed: false,
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

/// Immutable worker-owned SQLite snapshot with bounded reads.
pub struct WebSqliteSnapshot {
    database_name: Option<String>,
    session_id: String,
    byte_len: u64,
    reading_started: bool,
    closed: bool,
}

impl WebSqliteSnapshot {
    pub fn byte_len(&self) -> u64 {
        self.byte_len
    }

    /// Queries the immutable snapshot before chunk reading begins.
    pub async fn query(&mut self, query: SqlQuery) -> Result<SqlRows, SqlError> {
        if self.closed || self.reading_started {
            return Err(SqlError::new(
                SqlErrorKind::InvalidParameter,
                "Web SQLite snapshot queries must run before chunk reads",
            ));
        }
        request(
            self.database_name.as_deref(),
            "sqlite_snapshot_query",
            SnapshotQueryRequest {
                session_id: self.session_id.clone(),
                query,
            },
        )
        .await
        .map_err(sql_error)
    }

    /// Reads the next chunk. Requests are capped at
    /// [`MAX_WEB_SQLITE_STREAM_CHUNK_BYTES`]. EOF removes the worker snapshot.
    pub async fn read_chunk(&mut self, max_bytes: usize) -> Result<Option<Vec<u8>>, SqlError> {
        if max_bytes == 0 || max_bytes > MAX_WEB_SQLITE_STREAM_CHUNK_BYTES {
            return Err(SqlError::new(
                SqlErrorKind::InvalidParameter,
                format!(
                    "Web SQLite snapshot chunk size must be within 1..={MAX_WEB_SQLITE_STREAM_CHUNK_BYTES} bytes"
                ),
            ));
        }
        if self.closed {
            return Ok(None);
        }
        self.reading_started = true;
        let result: SnapshotChunkResponse = request(
            self.database_name.as_deref(),
            "sqlite_snapshot_read",
            SnapshotChunkRequest {
                session_id: self.session_id.clone(),
                max_bytes: max_bytes as u32,
            },
        )
        .await
        .map_err(sql_error)?;
        if result.done {
            self.closed = true;
        }
        if result.bytes.len() > max_bytes || (result.bytes.is_empty() && !result.done) {
            return Err(SqlError::new(
                SqlErrorKind::InvalidParameter,
                "Web SQLite snapshot worker returned an invalid chunk size",
            ));
        }
        if result.bytes.is_empty() && result.done {
            Ok(None)
        } else {
            Ok(Some(result.bytes))
        }
    }

    pub async fn cancel(&mut self) -> Result<(), SqlError> {
        if self.closed {
            return Ok(());
        }
        request::<_, ()>(
            self.database_name.as_deref(),
            "sqlite_snapshot_cancel",
            SessionIdRequest {
                session_id: self.session_id.clone(),
            },
        )
        .await
        .map_err(sql_error)?;
        self.closed = true;
        Ok(())
    }
}

impl Drop for WebSqliteSnapshot {
    fn drop(&mut self) {
        if self.closed {
            return;
        }
        spawn_session_cancel(
            self.database_name.clone(),
            "sqlite_snapshot_cancel",
            self.session_id.clone(),
        );
    }
}

/// Bounded writer for a worker-owned SQLite import staging file.
pub struct WebSqliteImport {
    database_name: Option<String>,
    session_id: String,
    closed: bool,
}

impl WebSqliteImport {
    pub async fn write_chunk(&mut self, bytes: &[u8]) -> Result<u64, SqlError> {
        if self.closed {
            return Err(SqlError::new(
                SqlErrorKind::InvalidParameter,
                "Web SQLite import staging session is closed",
            ));
        }
        if bytes.is_empty() || bytes.len() > MAX_WEB_SQLITE_STREAM_CHUNK_BYTES {
            return Err(SqlError::new(
                SqlErrorKind::InvalidParameter,
                format!(
                    "Web SQLite import chunks must contain 1..={MAX_WEB_SQLITE_STREAM_CHUNK_BYTES} bytes"
                ),
            ));
        }
        let result: ImportChunkResponse = request(
            self.database_name.as_deref(),
            "sqlite_import_write",
            ImportChunkRequest {
                session_id: self.session_id.clone(),
                bytes: bytes.to_vec(),
            },
        )
        .await
        .map_err(sql_error)?;
        if result.bytes_written as usize != bytes.len() {
            return Err(SqlError::new(
                SqlErrorKind::InvalidParameter,
                "Web SQLite import worker wrote an unexpected byte count",
            ));
        }
        Ok(result.bytes_written)
    }

    /// Finishes and validates the staged database without replacing the target.
    pub async fn finish_stage(&mut self) -> Result<WebSqliteStagedDatabase, SqlError> {
        if self.closed {
            return Err(SqlError::new(
                SqlErrorKind::InvalidParameter,
                "Web SQLite import staging session is closed",
            ));
        }
        let result: SessionIdResponse = request(
            self.database_name.as_deref(),
            "sqlite_import_finish_stage",
            SessionIdRequest {
                session_id: self.session_id.clone(),
            },
        )
        .await
        .map_err(sql_error)?;
        self.closed = true;
        Ok(WebSqliteStagedDatabase {
            database_name: self.database_name.clone(),
            session_id: result.session_id,
            closed: Cell::new(false),
        })
    }

    pub async fn cancel(&mut self) -> Result<(), SqlError> {
        if self.closed {
            return Ok(());
        }
        request::<_, ()>(
            self.database_name.as_deref(),
            "sqlite_import_cancel",
            SessionIdRequest {
                session_id: self.session_id.clone(),
            },
        )
        .await
        .map_err(sql_error)?;
        self.closed = true;
        Ok(())
    }
}

impl Drop for WebSqliteImport {
    fn drop(&mut self) {
        if self.closed {
            return;
        }
        spawn_session_cancel(
            self.database_name.clone(),
            "sqlite_import_cancel",
            self.session_id.clone(),
        );
    }
}

/// Read-only validated SQLite file produced by [`WebSqliteImport::finish_stage`].
pub struct WebSqliteStagedDatabase {
    database_name: Option<String>,
    session_id: String,
    closed: Cell<bool>,
}

impl WebSqliteStagedDatabase {
    /// Queries the imported file without changing it.
    pub async fn query(&self, query: SqlQuery) -> Result<SqlRows, SqlError> {
        if self.closed.get() {
            return Err(SqlError::new(
                SqlErrorKind::InvalidParameter,
                "Web SQLite staged database is closed",
            ));
        }
        request(
            self.database_name.as_deref(),
            "sqlite_stage_query",
            SnapshotQueryRequest {
                session_id: self.session_id.clone(),
                query,
            },
        )
        .await
        .map_err(sql_error)
    }

    /// Replaces this store's named database atomically with the staged image.
    /// Use only after application-level identity and schema validation. Existing
    /// PersonalDB workspaces should instead attach the stage to their own restore
    /// transaction and preserve PersonalDB's identity and restore rules.
    pub async fn install_into_target(&self) -> Result<(), SqlError> {
        if self.closed.get() {
            return Err(SqlError::new(
                SqlErrorKind::InvalidParameter,
                "Web SQLite staged database is closed",
            ));
        }
        request::<_, ()>(
            self.database_name.as_deref(),
            "sqlite_stage_install",
            SessionIdRequest {
                session_id: self.session_id.clone(),
            },
        )
        .await
        .map_err(sql_error)?;
        self.closed.set(true);
        Ok(())
    }

    /// Installs this image only if the named database is still empty when the
    /// worker begins the atomic replacement. The Fission `fission` table and
    /// SQLite-owned `sqlite_*` objects are permitted. This closes the race
    /// between checking a new workspace and finishing a potentially long import.
    /// If cancelled before SQLite finishes the backup, `sqlite3_backup_finish`
    /// rolls back the destination transaction. A backup-finish error is returned
    /// because SQLite may not have been able to complete that rollback.
    pub async fn install_into_empty_target(&self) -> Result<(), SqlError> {
        if self.closed.get() {
            return Err(SqlError::new(
                SqlErrorKind::InvalidParameter,
                "Web SQLite staged database is closed",
            ));
        }
        request::<_, ()>(
            self.database_name.as_deref(),
            "sqlite_stage_install_empty",
            SessionIdRequest {
                session_id: self.session_id.clone(),
            },
        )
        .await
        .map_err(sql_error)?;
        self.closed.set(true);
        Ok(())
    }

    /// Cancels this staged image, including an install currently in progress.
    /// The install future then completes with an error after SQLite rolls back
    /// its incomplete destination backup.
    pub async fn cancel(&self) -> Result<(), SqlError> {
        if self.closed.get() {
            return Ok(());
        }
        request::<_, ()>(
            self.database_name.as_deref(),
            "sqlite_stage_cancel",
            SessionIdRequest {
                session_id: self.session_id.clone(),
            },
        )
        .await
        .map_err(sql_error)?;
        self.closed.set(true);
        Ok(())
    }
}

impl Drop for WebSqliteStagedDatabase {
    fn drop(&mut self) {
        if self.closed.get() {
            return;
        }
        spawn_session_cancel(
            self.database_name.clone(),
            "sqlite_stage_cancel",
            self.session_id.clone(),
        );
    }
}

fn valid_sqlite_alias(alias: &str) -> bool {
    let mut bytes = alias.bytes();
    matches!(bytes.next(), Some(b'a'..=b'z' | b'A'..=b'Z' | b'_'))
        && bytes.all(|byte| matches!(byte, b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'_'))
        && alias.len() <= 64
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

    /// Attaches a validated import stage to this transaction under an opaque
    /// SQL alias. The worker makes the attached schema read-only and detaches it
    /// automatically after commit or rollback.
    pub async fn attach_staged_database(
        &mut self,
        staged: &WebSqliteStagedDatabase,
        alias: impl Into<String>,
    ) -> Result<(), SqlError> {
        self.ensure_open()?;
        if staged.closed.get() || self.database_name != staged.database_name {
            return Err(SqlError::new(
                SqlErrorKind::InvalidParameter,
                "staged database must be open in the same named Web SQLite store",
            ));
        }
        let alias = alias.into();
        if !valid_sqlite_alias(&alias) {
            return Err(SqlError::new(
                SqlErrorKind::InvalidParameter,
                "Web SQLite staged database aliases must be simple SQL identifiers",
            ));
        }
        request::<_, ()>(
            self.database_name.as_deref(),
            "sql_tx_stage_attach",
            TransactionStageAttachRequest {
                transaction_id: self.id.clone(),
                stage_id: staged.session_id.clone(),
                alias,
            },
        )
        .await
        .map_err(sql_error)
    }

    /// Detaches a staged database from this transaction. Commit and rollback
    /// also detach every remaining stage.
    pub async fn detach_staged_database(
        &mut self,
        alias: impl Into<String>,
    ) -> Result<(), SqlError> {
        self.ensure_open()?;
        let alias = alias.into();
        if !valid_sqlite_alias(&alias) {
            return Err(SqlError::new(
                SqlErrorKind::InvalidParameter,
                "Web SQLite staged database aliases must be simple SQL identifiers",
            ));
        }
        request::<_, ()>(
            self.database_name.as_deref(),
            "sql_tx_stage_detach",
            TransactionStageDetachRequest {
                transaction_id: self.id.clone(),
                alias,
            },
        )
        .await
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
struct SessionBeginRequest {
    begin_token: String,
}

#[derive(Serialize)]
struct SessionBeginTokenRequest {
    begin_token: String,
}

#[derive(Deserialize)]
struct SessionBeginResponse {
    session_id: String,
    #[serde(default)]
    byte_len: Option<String>,
}

#[derive(Serialize)]
struct SessionIdRequest {
    session_id: String,
}

#[derive(Serialize)]
struct SnapshotQueryRequest {
    session_id: String,
    query: SqlQuery,
}

#[derive(Serialize)]
struct SnapshotChunkRequest {
    session_id: String,
    max_bytes: u32,
}

#[derive(Deserialize)]
struct SnapshotChunkResponse {
    #[serde(with = "serde_byte_vec")]
    bytes: Vec<u8>,
    done: bool,
}

#[derive(Serialize)]
struct ImportChunkRequest {
    session_id: String,
    #[serde(with = "serde_byte_vec")]
    bytes: Vec<u8>,
}

#[derive(Deserialize)]
struct ImportChunkResponse {
    bytes_written: u64,
}

mod serde_byte_vec {
    use super::{SeqAccess, Serializer, Visitor};
    use serde::de::Error;
    use serde::Deserializer;
    use std::fmt;

    pub fn serialize<S>(bytes: &Vec<u8>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_bytes(bytes)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Vec<u8>, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct ByteVecVisitor;

        impl<'de> Visitor<'de> for ByteVecVisitor {
            type Value = Vec<u8>;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("a byte buffer")
            }

            fn visit_bytes<E: Error>(self, bytes: &[u8]) -> Result<Self::Value, E> {
                Ok(bytes.to_vec())
            }

            fn visit_byte_buf<E: Error>(self, bytes: Vec<u8>) -> Result<Self::Value, E> {
                Ok(bytes)
            }

            fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Self::Value, A::Error> {
                let mut bytes = Vec::with_capacity(seq.size_hint().unwrap_or_default());
                while let Some(byte) = seq.next_element()? {
                    bytes.push(byte);
                }
                Ok(bytes)
            }
        }

        deserializer.deserialize_bytes(ByteVecVisitor)
    }
}

#[derive(Serialize)]
struct TransactionStageAttachRequest {
    transaction_id: String,
    stage_id: String,
    alias: String,
}

#[derive(Serialize)]
struct TransactionStageDetachRequest {
    transaction_id: String,
    alias: String,
}

/// An in-flight begin can outlive the Rust future. Retain the browser promise
/// until the worker returns a session ID, then cancel it if the future was
/// dropped before it could take ownership.
struct PendingSessionBeginRollback {
    database_name: Option<String>,
    cancel_operation: &'static str,
    pending_cancel_operation: &'static str,
    begin_token: String,
    promise: Option<js_sys::Promise>,
}

impl PendingSessionBeginRollback {
    fn disarm(mut self) {
        self.promise = None;
    }
}

impl Drop for PendingSessionBeginRollback {
    fn drop(&mut self) {
        let Some(promise) = self.promise.take() else {
            return;
        };
        let database_name = self.database_name.clone();
        let cancel_operation = self.cancel_operation;
        let pending_cancel_operation = self.pending_cancel_operation;
        let begin_token = self.begin_token.clone();
        wasm_bindgen_futures::spawn_local(async move {
            let _ = request::<_, ()>(
                database_name.as_deref(),
                pending_cancel_operation,
                SessionBeginTokenRequest {
                    begin_token: begin_token.clone(),
                },
            )
            .await;
            if let Ok(result) = await_response::<SessionBeginResponse>(promise).await {
                let _ = request::<_, ()>(
                    database_name.as_deref(),
                    cancel_operation,
                    SessionIdRequest {
                        session_id: result.session_id,
                    },
                )
                .await;
            }
        });
    }
}

fn session_begin_token() -> String {
    format!(
        "{:016x}-{:08x}",
        js_sys::Date::now() as u64,
        (js_sys::Math::random() * u32::MAX as f64) as u32
    )
}

fn spawn_session_cancel(
    database_name: Option<String>,
    operation: &'static str,
    session_id: String,
) {
    wasm_bindgen_futures::spawn_local(async move {
        let _ = request::<_, ()>(
            database_name.as_deref(),
            operation,
            SessionIdRequest { session_id },
        )
        .await;
    });
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
