use fission_store::{
    SqlColumn, SqlError, SqlErrorKind, SqlExecuteResult, SqlParameters, SqlQuery, SqlRow, SqlRows,
    SqlStatement, SqlStepResult, SqlStoreProvider, SqlTransaction, SqlTransactionResult,
    SqlTransactionStep, SqlValue, StoreAddress, StoreBatch, StoreBatchOperation, StoreBatchResult,
    StoreEntry, StoreError, StoreErrorKind, StoreFuture, StoreGet, StoreListPrefix, StoreProvider,
    StoreRemove, StoreSet, StoreValue,
};
use rusqlite::types::{Value, ValueRef};
use rusqlite::{Connection, ErrorCode, Statement};
use std::path::Path;
use std::sync::{Arc, Mutex, MutexGuard};

#[derive(Clone)]
pub struct SqliteStore {
    connection: Arc<Mutex<Connection>>,
}

impl SqliteStore {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, SqlError> {
        let connection = Connection::open(path).map_err(map_sql_error)?;
        Self::from_connection(connection)
    }

    pub fn open_in_memory() -> Result<Self, SqlError> {
        let connection = Connection::open_in_memory().map_err(map_sql_error)?;
        Self::from_connection(connection)
    }

    pub fn from_connection(connection: Connection) -> Result<Self, SqlError> {
        connection
            .execute_batch(crate::FISSION_STORE_SCHEMA)
            .map_err(map_sql_error)?;
        Ok(Self {
            connection: Arc::new(Mutex::new(connection)),
        })
    }

    fn connection(&self) -> Result<MutexGuard<'_, Connection>, SqlError> {
        self.connection
            .lock()
            .map_err(|_| SqlError::new(SqlErrorKind::Backend, "SQLite connection lock poisoned"))
    }
}

impl StoreProvider for SqliteStore {
    fn get(&self, request: StoreGet) -> StoreFuture<Result<Option<StoreValue>, StoreError>> {
        let this = self.clone();
        Box::pin(async move {
            let connection = this.connection().map_err(sql_to_store_error)?;
            get_value(&connection, &request.address)
        })
    }

    fn set(&self, request: StoreSet) -> StoreFuture<Result<(), StoreError>> {
        let this = self.clone();
        Box::pin(async move {
            let connection = this.connection().map_err(sql_to_store_error)?;
            set_value(&connection, &request)?;
            Ok(())
        })
    }

    fn remove(&self, request: StoreRemove) -> StoreFuture<Result<bool, StoreError>> {
        let this = self.clone();
        Box::pin(async move {
            let connection = this.connection().map_err(sql_to_store_error)?;
            remove_value(&connection, &request.address)
        })
    }

    fn batch(&self, request: StoreBatch) -> StoreFuture<Result<StoreBatchResult, StoreError>> {
        let this = self.clone();
        Box::pin(async move {
            let mut connection = this.connection().map_err(sql_to_store_error)?;
            let transaction = connection.transaction().map_err(map_store_error)?;
            let mut result = StoreBatchResult::default();
            for operation in request.into_operations() {
                match operation {
                    StoreBatchOperation::Set(request) => {
                        set_value(&transaction, &request)?;
                        result.sets += 1;
                    }
                    StoreBatchOperation::Remove(request) => {
                        if remove_value(&transaction, &request.address)? {
                            result.removals += 1;
                        }
                    }
                }
            }
            transaction.commit().map_err(map_store_error)?;
            Ok(result)
        })
    }

    fn list_prefix(
        &self,
        request: StoreListPrefix,
    ) -> StoreFuture<Result<Vec<StoreEntry>, StoreError>> {
        let this = self.clone();
        Box::pin(async move {
            let connection = this.connection().map_err(sql_to_store_error)?;
            let (scope, owner) = request.scope.parts();
            let mut statement = connection
                .prepare(
                    "SELECT key, value, revision FROM fission \
                     WHERE scope = ?1 AND owner = ?2 AND namespace = ?3 \
                     AND substr(key, 1, length(?4)) = ?4 ORDER BY key",
                )
                .map_err(map_store_error)?;
            let rows = statement
                .query_map(
                    rusqlite::params![scope, owner, request.namespace, request.prefix],
                    |row| {
                        Ok((
                            row.get::<_, String>(0)?,
                            row.get::<_, Vec<u8>>(1)?,
                            row.get::<_, u64>(2)?,
                        ))
                    },
                )
                .map_err(map_store_error)?;
            let mut entries = Vec::new();
            for row in rows {
                let (key, value, revision) = row.map_err(map_store_error)?;
                entries.push(StoreEntry {
                    address: StoreAddress {
                        scope: request.scope.clone(),
                        namespace: request.namespace.clone(),
                        key,
                    },
                    value: StoreValue(value),
                    revision,
                });
            }
            Ok(entries)
        })
    }
}

impl SqlStoreProvider for SqliteStore {
    fn execute(&self, statement: SqlStatement) -> StoreFuture<Result<SqlExecuteResult, SqlError>> {
        let this = self.clone();
        Box::pin(async move {
            let connection = this.connection()?;
            execute_statement(&connection, &statement)
        })
    }

    fn query(&self, request: SqlQuery) -> StoreFuture<Result<SqlRows, SqlError>> {
        let this = self.clone();
        Box::pin(async move {
            let connection = this.connection()?;
            query_statement(&connection, &request.statement)
        })
    }

    fn transaction(
        &self,
        request: SqlTransaction,
    ) -> StoreFuture<Result<SqlTransactionResult, SqlError>> {
        let this = self.clone();
        Box::pin(async move {
            let mut connection = this.connection()?;
            let transaction = connection.transaction().map_err(map_sql_error)?;
            let mut results = Vec::with_capacity(request.steps().len());
            for step in request.into_steps() {
                match step {
                    SqlTransactionStep::Execute(statement) => results.push(SqlStepResult::Execute(
                        execute_statement(&transaction, &statement)?,
                    )),
                    SqlTransactionStep::Query(query) => results.push(SqlStepResult::Query(
                        query_statement(&transaction, &query.statement)?,
                    )),
                }
            }
            transaction.commit().map_err(map_sql_error)?;
            Ok(SqlTransactionResult { steps: results })
        })
    }
}

fn get_value(
    connection: &Connection,
    address: &StoreAddress,
) -> Result<Option<StoreValue>, StoreError> {
    let (scope, owner) = address.scope.parts();
    match connection.query_row(
        "SELECT value FROM fission WHERE scope = ?1 AND owner = ?2 AND namespace = ?3 AND key = ?4",
        rusqlite::params![scope, owner, address.namespace, address.key],
        |row| row.get::<_, Vec<u8>>(0),
    ) {
        Ok(value) => Ok(Some(StoreValue(value))),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(error) => Err(map_store_error(error)),
    }
}

fn set_value(connection: &Connection, request: &StoreSet) -> Result<(), StoreError> {
    let (scope, owner) = request.address.scope.parts();
    connection
        .execute(
            "INSERT INTO fission(scope, owner, namespace, key, value, revision) \
             VALUES (?1, ?2, ?3, ?4, ?5, 1) \
             ON CONFLICT(scope, owner, namespace, key) DO UPDATE SET \
             value = excluded.value, revision = fission.revision + 1",
            rusqlite::params![
                scope,
                owner,
                request.address.namespace,
                request.address.key,
                request.value.0
            ],
        )
        .map_err(map_store_error)?;
    Ok(())
}

fn remove_value(connection: &Connection, address: &StoreAddress) -> Result<bool, StoreError> {
    let (scope, owner) = address.scope.parts();
    connection
        .execute(
            "DELETE FROM fission WHERE scope = ?1 AND owner = ?2 AND namespace = ?3 AND key = ?4",
            rusqlite::params![scope, owner, address.namespace, address.key],
        )
        .map(|affected| affected > 0)
        .map_err(map_store_error)
}

fn execute_statement(
    connection: &Connection,
    request: &SqlStatement,
) -> Result<SqlExecuteResult, SqlError> {
    let mut statement = connection.prepare(&request.sql).map_err(map_sql_error)?;
    bind_parameters(&mut statement, &request.parameters)?;
    let affected_rows = statement.raw_execute().map_err(map_sql_error)? as u64;
    Ok(SqlExecuteResult {
        affected_rows,
        last_insert_rowid: Some(connection.last_insert_rowid()),
    })
}

fn query_statement(connection: &Connection, request: &SqlStatement) -> Result<SqlRows, SqlError> {
    let mut statement = connection.prepare(&request.sql).map_err(map_sql_error)?;
    let columns = statement
        .columns()
        .iter()
        .map(|column| SqlColumn {
            name: column.name().to_string(),
            declared_type: column.decl_type().map(ToString::to_string),
        })
        .collect::<Vec<_>>();
    bind_parameters(&mut statement, &request.parameters)?;
    let mut cursor = statement.raw_query();
    let mut rows = Vec::new();
    while let Some(row) = cursor.next().map_err(map_sql_error)? {
        let mut values = Vec::with_capacity(columns.len());
        for index in 0..columns.len() {
            values.push(value_from_ref(row.get_ref(index).map_err(map_sql_error)?));
        }
        rows.push(SqlRow::new(columns.clone(), values));
    }
    Ok(SqlRows { columns, rows })
}

fn bind_parameters(
    statement: &mut Statement<'_>,
    parameters: &SqlParameters,
) -> Result<(), SqlError> {
    match parameters {
        SqlParameters::None => Ok(()),
        SqlParameters::Positional(values) => {
            for (index, value) in values.iter().enumerate() {
                statement
                    .raw_bind_parameter(index + 1, value_to_rusqlite(value))
                    .map_err(map_sql_error)?;
            }
            Ok(())
        }
        SqlParameters::Named(values) => {
            for (name, value) in values {
                let index = statement
                    .parameter_index(name)
                    .map_err(map_sql_error)?
                    .ok_or_else(|| {
                        SqlError::new(
                            SqlErrorKind::InvalidParameter,
                            format!("SQL statement does not contain parameter `{name}`"),
                        )
                    })?;
                statement
                    .raw_bind_parameter(index, value_to_rusqlite(value))
                    .map_err(map_sql_error)?;
            }
            Ok(())
        }
    }
}

fn value_to_rusqlite(value: &SqlValue) -> Value {
    match value {
        SqlValue::Null => Value::Null,
        SqlValue::Integer(value) => Value::Integer(*value),
        SqlValue::Real(value) => Value::Real(*value),
        SqlValue::Text(value) => Value::Text(value.clone()),
        SqlValue::Blob(value) => Value::Blob(value.clone()),
    }
}

fn value_from_ref(value: ValueRef<'_>) -> SqlValue {
    match value {
        ValueRef::Null => SqlValue::Null,
        ValueRef::Integer(value) => SqlValue::Integer(value),
        ValueRef::Real(value) => SqlValue::Real(value),
        ValueRef::Text(value) => SqlValue::Text(String::from_utf8_lossy(value).into_owned()),
        ValueRef::Blob(value) => SqlValue::Blob(value.to_vec()),
    }
}

fn map_store_error(error: rusqlite::Error) -> StoreError {
    let sql = map_sql_error(error);
    let kind = match sql.kind {
        SqlErrorKind::Unavailable => StoreErrorKind::Unavailable,
        SqlErrorKind::Busy => StoreErrorKind::Busy,
        SqlErrorKind::ReadOnly => StoreErrorKind::ReadOnly,
        SqlErrorKind::InvalidParameter | SqlErrorKind::InvalidSql => StoreErrorKind::InvalidRequest,
        _ => StoreErrorKind::Backend,
    };
    StoreError::new(kind, sql.message)
}

fn map_sql_error(error: rusqlite::Error) -> SqlError {
    let (kind, code) = match &error {
        rusqlite::Error::SqliteFailure(failure, _) => {
            let kind = match failure.code {
                ErrorCode::DatabaseBusy | ErrorCode::DatabaseLocked => SqlErrorKind::Busy,
                ErrorCode::ReadOnly => SqlErrorKind::ReadOnly,
                ErrorCode::ConstraintViolation => SqlErrorKind::Constraint,
                _ => SqlErrorKind::Backend,
            };
            (kind, Some(failure.extended_code))
        }
        rusqlite::Error::InvalidQuery => (SqlErrorKind::InvalidSql, None),
        rusqlite::Error::InvalidParameterName(_) | rusqlite::Error::InvalidParameterCount(_, _) => {
            (SqlErrorKind::InvalidParameter, None)
        }
        _ => (SqlErrorKind::Backend, None),
    };
    let mut mapped = SqlError::new(kind, error.to_string());
    mapped.sqlite_code = code;
    mapped
}

fn sql_to_store_error(error: SqlError) -> StoreError {
    let kind = match error.kind {
        SqlErrorKind::Unavailable => StoreErrorKind::Unavailable,
        SqlErrorKind::Busy => StoreErrorKind::Busy,
        SqlErrorKind::ReadOnly => StoreErrorKind::ReadOnly,
        SqlErrorKind::InvalidParameter | SqlErrorKind::InvalidSql => StoreErrorKind::InvalidRequest,
        _ => StoreErrorKind::Backend,
    };
    StoreError::new(kind, error.message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use fission_store::StoreScope;

    fn run<T>(future: StoreFuture<T>) -> T {
        futures_lite::future::block_on(future)
    }

    #[test]
    fn typed_store_scopes_and_atomic_batches_share_the_reserved_table() {
        let store = SqliteStore::open_in_memory().unwrap();
        let application = StoreAddress::new("settings", "theme");
        let user = StoreAddress::new("settings", "theme").in_scope(StoreScope::user("42"));

        let mut batch = StoreBatch::new();
        batch
            .set(StoreSet {
                address: application.clone(),
                value: b"dark".to_vec().into(),
            })
            .set(StoreSet {
                address: user.clone(),
                value: b"light".to_vec().into(),
            });
        assert_eq!(run(store.batch(batch)).unwrap().sets, 2);

        assert_eq!(
            run(store.get(StoreGet {
                address: application
            }))
            .unwrap()
            .unwrap()
            .0,
            b"dark"
        );
        assert_eq!(
            run(store.get(StoreGet { address: user }))
                .unwrap()
                .unwrap()
                .0,
            b"light"
        );
    }

    #[test]
    fn arbitrary_sql_and_incrementally_built_transactions_execute() {
        let store = SqliteStore::open_in_memory().unwrap();
        run(store.execute(SqlStatement::new(
            "CREATE TABLE projects(id INTEGER PRIMARY KEY, name TEXT NOT NULL)",
        )))
        .unwrap();

        let mut transaction = SqlTransaction::new();
        transaction
            .execute(SqlStatement::new("INSERT INTO projects(name) VALUES (?1)").bind("Fission"));
        fn add_second(transaction: &mut SqlTransaction) {
            transaction.execute(
                SqlStatement::new("INSERT INTO projects(name) VALUES (:name)")
                    .bind_named(":name", "Worka"),
            );
        }
        add_second(&mut transaction);
        transaction.query(SqlStatement::new(
            "SELECT id, name FROM projects ORDER BY id",
        ));

        let result = run(store.transaction(transaction)).unwrap();
        let SqlStepResult::Query(rows) = &result.steps[2] else {
            panic!("third result must be the query")
        };
        assert_eq!(rows.rows[0].get::<String>("name").unwrap(), "Fission");
        assert_eq!(rows.rows[1].get::<String>("name").unwrap(), "Worka");
    }
}
