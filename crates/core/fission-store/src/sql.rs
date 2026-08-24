use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// SQLite's portable value model.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum SqlValue {
    Null,
    Integer(i64),
    Real(f64),
    Text(String),
    Blob(Vec<u8>),
}

macro_rules! integer_value {
    ($($ty:ty),* $(,)?) => {$(
        impl From<$ty> for SqlValue {
            fn from(value: $ty) -> Self { Self::Integer(value as i64) }
        }
    )*};
}

integer_value!(i8, i16, i32, i64, u8, u16, u32);

impl From<f32> for SqlValue {
    fn from(value: f32) -> Self {
        Self::Real(f64::from(value))
    }
}

impl From<f64> for SqlValue {
    fn from(value: f64) -> Self {
        Self::Real(value)
    }
}

impl From<String> for SqlValue {
    fn from(value: String) -> Self {
        Self::Text(value)
    }
}

impl From<&str> for SqlValue {
    fn from(value: &str) -> Self {
        Self::Text(value.to_string())
    }
}

impl From<Vec<u8>> for SqlValue {
    fn from(value: Vec<u8>) -> Self {
        Self::Blob(value)
    }
}

impl<T: Into<SqlValue>> From<Option<T>> for SqlValue {
    fn from(value: Option<T>) -> Self {
        value.map(Into::into).unwrap_or(Self::Null)
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub enum SqlParameters {
    #[default]
    None,
    Positional(Vec<SqlValue>),
    Named(Vec<(String, SqlValue)>),
}

/// Parameterized SQL which can cross native and Web worker boundaries.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SqlStatement {
    pub sql: String,
    pub parameters: SqlParameters,
}

impl SqlStatement {
    pub fn new(sql: impl Into<String>) -> Self {
        Self {
            sql: sql.into(),
            parameters: SqlParameters::None,
        }
    }

    pub fn bind(mut self, value: impl Into<SqlValue>) -> Self {
        self.push_bind(value);
        self
    }

    pub fn push_bind(&mut self, value: impl Into<SqlValue>) -> &mut Self {
        match &mut self.parameters {
            SqlParameters::None => {
                self.parameters = SqlParameters::Positional(vec![value.into()]);
            }
            SqlParameters::Positional(values) => values.push(value.into()),
            SqlParameters::Named(_) => {
                panic!("cannot mix positional and named SQL parameters")
            }
        }
        self
    }

    pub fn bind_named(mut self, name: impl Into<String>, value: impl Into<SqlValue>) -> Self {
        self.push_named(name, value);
        self
    }

    pub fn push_named(&mut self, name: impl Into<String>, value: impl Into<SqlValue>) -> &mut Self {
        match &mut self.parameters {
            SqlParameters::None => {
                self.parameters = SqlParameters::Named(vec![(name.into(), value.into())]);
            }
            SqlParameters::Named(values) => values.push((name.into(), value.into())),
            SqlParameters::Positional(_) => {
                panic!("cannot mix named and positional SQL parameters")
            }
        }
        self
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SqlQuery {
    pub statement: SqlStatement,
}

impl From<SqlStatement> for SqlQuery {
    fn from(statement: SqlStatement) -> Self {
        Self { statement }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SqlColumn {
    pub name: String,
    pub declared_type: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SqlRow {
    columns: Vec<SqlColumn>,
    values: Vec<SqlValue>,
}

impl SqlRow {
    pub fn new(columns: Vec<SqlColumn>, values: Vec<SqlValue>) -> Self {
        Self { columns, values }
    }

    pub fn columns(&self) -> &[SqlColumn] {
        &self.columns
    }

    pub fn values(&self) -> &[SqlValue] {
        &self.values
    }

    pub fn get<T: FromSqlValue>(&self, name: &str) -> Result<T, SqlError> {
        let index = self
            .columns
            .iter()
            .position(|column| column.name == name)
            .ok_or_else(|| SqlError::invalid_column(name))?;
        self.get_index(index)
    }

    pub fn get_index<T: FromSqlValue>(&self, index: usize) -> Result<T, SqlError> {
        let value = self
            .values
            .get(index)
            .ok_or_else(|| SqlError::invalid_column(index.to_string()))?;
        T::from_sql_value(value)
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct SqlRows {
    pub columns: Vec<SqlColumn>,
    pub rows: Vec<SqlRow>,
}

impl IntoIterator for SqlRows {
    type Item = SqlRow;
    type IntoIter = std::vec::IntoIter<SqlRow>;

    fn into_iter(self) -> Self::IntoIter {
        self.rows.into_iter()
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SqlExecuteResult {
    pub affected_rows: u64,
    pub last_insert_rowid: Option<i64>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum SqlTransactionStep {
    Execute(SqlStatement),
    Query(SqlQuery),
}

/// A serializable SQLite transaction assembled across application modules.
///
/// Mutating methods intentionally take `&mut self`, allowing helpers far from
/// the creation site to add work before the completed transaction is dispatched.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct SqlTransaction {
    steps: Vec<SqlTransactionStep>,
}

impl SqlTransaction {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn execute(&mut self, statement: SqlStatement) -> &mut Self {
        self.steps.push(SqlTransactionStep::Execute(statement));
        self
    }

    pub fn query(&mut self, statement: impl Into<SqlQuery>) -> &mut Self {
        self.steps.push(SqlTransactionStep::Query(statement.into()));
        self
    }

    pub fn with_execute(mut self, statement: SqlStatement) -> Self {
        self.execute(statement);
        self
    }

    pub fn with_query(mut self, statement: impl Into<SqlQuery>) -> Self {
        self.query(statement);
        self
    }

    pub fn extend(&mut self, other: impl IntoIterator<Item = SqlTransactionStep>) -> &mut Self {
        self.steps.extend(other);
        self
    }

    pub fn steps(&self) -> &[SqlTransactionStep] {
        &self.steps
    }

    pub fn into_steps(self) -> Vec<SqlTransactionStep> {
        self.steps
    }
}

impl Extend<SqlTransactionStep> for SqlTransaction {
    fn extend<T: IntoIterator<Item = SqlTransactionStep>>(&mut self, iter: T) {
        self.steps.extend(iter);
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum SqlStepResult {
    Execute(SqlExecuteResult),
    Query(SqlRows),
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct SqlTransactionResult {
    pub steps: Vec<SqlStepResult>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SqlMigration {
    pub version: u64,
    pub name: String,
    pub sql: String,
}

impl SqlMigration {
    pub fn new(version: u64, name: impl Into<String>, sql: impl Into<String>) -> Self {
        Self {
            version,
            name: name.into(),
            sql: sql.into(),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SqlMigrations {
    migrations: BTreeMap<u64, SqlMigration>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SqlMigrationResult {
    pub previous_version: u64,
    pub current_version: u64,
    pub applied: u64,
}

impl SqlMigrations {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add(&mut self, migration: SqlMigration) -> Result<&mut Self, SqlError> {
        if self.migrations.contains_key(&migration.version) {
            return Err(SqlError::new(
                SqlErrorKind::Migration,
                format!("duplicate SQL migration version {}", migration.version),
            ));
        }
        self.migrations.insert(migration.version, migration);
        Ok(self)
    }

    pub fn iter(&self) -> impl Iterator<Item = &SqlMigration> {
        self.migrations.values()
    }
}

pub trait FromSqlValue: Sized {
    fn from_sql_value(value: &SqlValue) -> Result<Self, SqlError>;
}

impl FromSqlValue for SqlValue {
    fn from_sql_value(value: &SqlValue) -> Result<Self, SqlError> {
        Ok(value.clone())
    }
}

impl FromSqlValue for i64 {
    fn from_sql_value(value: &SqlValue) -> Result<Self, SqlError> {
        match value {
            SqlValue::Integer(value) => Ok(*value),
            _ => Err(SqlError::type_mismatch("integer", value)),
        }
    }
}

impl FromSqlValue for f64 {
    fn from_sql_value(value: &SqlValue) -> Result<Self, SqlError> {
        match value {
            SqlValue::Real(value) => Ok(*value),
            SqlValue::Integer(value) => Ok(*value as f64),
            _ => Err(SqlError::type_mismatch("real", value)),
        }
    }
}

impl FromSqlValue for String {
    fn from_sql_value(value: &SqlValue) -> Result<Self, SqlError> {
        match value {
            SqlValue::Text(value) => Ok(value.clone()),
            _ => Err(SqlError::type_mismatch("text", value)),
        }
    }
}

impl FromSqlValue for Vec<u8> {
    fn from_sql_value(value: &SqlValue) -> Result<Self, SqlError> {
        match value {
            SqlValue::Blob(value) => Ok(value.clone()),
            _ => Err(SqlError::type_mismatch("blob", value)),
        }
    }
}

impl<T: FromSqlValue> FromSqlValue for Option<T> {
    fn from_sql_value(value: &SqlValue) -> Result<Self, SqlError> {
        match value {
            SqlValue::Null => Ok(None),
            value => T::from_sql_value(value).map(Some),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SqlErrorKind {
    Unavailable,
    Busy,
    Constraint,
    ReadOnly,
    InvalidSql,
    InvalidParameter,
    InvalidColumn,
    TypeMismatch,
    Transaction,
    Migration,
    Backend,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SqlError {
    pub kind: SqlErrorKind,
    pub message: String,
    pub sqlite_code: Option<i32>,
}

impl SqlError {
    pub fn new(kind: SqlErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
            sqlite_code: None,
        }
    }

    pub fn with_sqlite_code(mut self, code: i32) -> Self {
        self.sqlite_code = Some(code);
        self
    }

    fn invalid_column(column: impl Into<String>) -> Self {
        Self::new(
            SqlErrorKind::InvalidColumn,
            format!("SQL result does not contain column `{}`", column.into()),
        )
    }

    fn type_mismatch(expected: &str, value: &SqlValue) -> Self {
        Self::new(
            SqlErrorKind::TypeMismatch,
            format!("expected SQL {expected}, received {value:?}"),
        )
    }
}

impl std::fmt::Display for SqlError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}", self.message)
    }
}

impl std::error::Error for SqlError {}

#[cfg(test)]
mod tests {
    use super::*;

    fn add_audit_write(transaction: &mut SqlTransaction) {
        transaction
            .execute(SqlStatement::new("INSERT INTO audit(message) VALUES (?1)").bind("updated"));
    }

    #[test]
    fn transaction_is_passable_and_extensible_after_construction() {
        let mut transaction = SqlTransaction::new();
        transaction.execute(
            SqlStatement::new("UPDATE accounts SET balance = balance - ?1 WHERE id = ?2")
                .bind(10_i64)
                .bind("source"),
        );
        add_audit_write(&mut transaction);
        transaction.query(SqlStatement::new("SELECT total_changes() AS changes"));

        assert_eq!(transaction.steps().len(), 3);
        let encoded = serde_json::to_vec(&transaction).unwrap();
        assert_eq!(
            serde_json::from_slice::<SqlTransaction>(&encoded).unwrap(),
            transaction
        );
    }

    #[test]
    fn rows_support_named_and_indexed_typed_access() {
        let row = SqlRow::new(
            vec![
                SqlColumn {
                    name: "id".into(),
                    declared_type: Some("INTEGER".into()),
                },
                SqlColumn {
                    name: "name".into(),
                    declared_type: Some("TEXT".into()),
                },
            ],
            vec![SqlValue::Integer(7), SqlValue::Text("Fission".into())],
        );
        assert_eq!(row.get::<i64>("id").unwrap(), 7);
        assert_eq!(row.get_index::<String>(1).unwrap(), "Fission");
    }
}
