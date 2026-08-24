use fission::sql::{SqlStatement, SqlTransaction};

pub fn new_project(name: impl Into<String>) -> SqlTransaction {
    SqlTransaction::new()
        .with_execute(SqlStatement::new("INSERT INTO projects(name) VALUES (?1)").bind(name.into()))
}

/// A separate application module can add work to the caller's transaction.
pub fn append_audit(transaction: &mut SqlTransaction, message: impl Into<String>) {
    transaction
        .execute(SqlStatement::new("INSERT INTO audit(message) VALUES (?1)").bind(message.into()));
}
