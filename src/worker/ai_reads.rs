//! The AI's read tools, run on its own connection. Statements are checked first and
//! run inside a read-only transaction that always rolls back, with a row cap.

use super::handlers::explain_statements;
use crate::ai::{clip, format_objects, format_sets, format_table, MAX_ROWS};
use crate::db::{DbError, Driver};
use crate::domain::{DbRead, ResultSet};
use crate::sql::{check_read, CapRows, ReadKind};

const STATEMENT_TIMEOUT_SECS: u32 = 30;

/// The answer to one read tool, as text for the AI.
pub(super) fn ai_read(d: &mut dyn Driver, db: &str, schema: &str, read: DbRead) -> Result<String, DbError> {
    match read {
        DbRead::ListObjects => Ok(format_objects(&d.tables(db)?, &d.routines(db)?)),
        DbRead::DescribeTable { table } => Ok(format_table(&d.columns(db, &table)?, &d.table_parts(db, &table)?)),
        DbRead::GetDdl { name, is_view } => Ok(clip(&d.ddl(db, &name, is_view)?)),
        DbRead::Select { sql } => select(d, (db, schema), &sql),
        DbRead::Explain { sql } => explain(d, (db, schema), &sql),
    }
}

fn select(d: &mut dyn Driver, (db, schema): (&str, &str), sql: &str) -> Result<String, DbError> {
    let dialect = d.dialect();
    let stmt = check_read(dialect, sql, ReadKind::Select).map_err(DbError::Invalid)?;
    read_only_session(d, db)?;
    if let Some(timeout) = dialect.statement_timeout(STATEMENT_TIMEOUT_SECS) {
        // Best effort: MariaDB and older MySQL name this setting differently, and a
        // read without a timeout is still only a read.
        let _ = d.query(db, &timeout);
    }
    d.query(db, dialect.read_only_begin())?;
    let result = in_schema(d, (db, schema), |d| capped(d, db, &stmt));
    // Always rolled back. It can only fail on a session that is already broken, and
    // the statement's own result is what the AI needs to see.
    let _ = d.query(db, "ROLLBACK");
    result.map(|sets| format_sets(&sets))
}

/// Runs `stmt` asking the server for one row more than the AI is shown, so the text
/// can say it was cut.
fn capped(d: &mut dyn Driver, db: &str, stmt: &str) -> Result<Vec<ResultSet>, DbError> {
    match d.dialect().cap_rows(stmt, MAX_ROWS + 1) {
        CapRows::Wrapped(sql) => d.query(db, &sql),
        CapRows::Session { on, off } => {
            d.query(db, &on)?;
            let result = d.query(db, stmt);
            // Turned off for whatever runs next; a failure means the session is gone.
            let _ = d.query(db, off);
            result
        }
    }
}

/// Plans only; nothing runs, so no transaction is needed.
fn explain(d: &mut dyn Driver, (db, schema): (&str, &str), sql: &str) -> Result<String, DbError> {
    let stmt = check_read(d.dialect(), sql, ReadKind::Explain).map_err(DbError::Invalid)?;
    read_only_session(d, db)?;
    let (sets, error) = in_schema(d, (db, schema), |d| explain_statements(d, db, &stmt));
    match error {
        Some((_, e)) => Err(e),
        None => Ok(format_sets(&sets)),
    }
}

/// Makes the whole AI session read-only where the server can, so nothing a statement
/// slips past the guard can write. Not best effort: without it, nothing runs.
fn read_only_session(d: &mut dyn Driver, db: &str) -> Result<(), DbError> {
    match d.dialect().read_only_session() {
        Some(sql) => d.query(db, sql).map(|_| ()),
        None => Ok(()),
    }
}

/// Runs `f` with unqualified names resolving in `schema` first, as query tabs do.
fn in_schema<T>(d: &mut dyn Driver, (db, schema): (&str, &str), f: impl FnOnce(&mut dyn Driver) -> T) -> T {
    let dialect = d.dialect();
    let set = dialect.use_schema(schema);
    if let Some(sql) = &set {
        // If this fails the names resolve as by default, and the statement says so.
        let _ = d.query(db, sql);
    }
    let out = f(d);
    if let (Some(_), Some(reset)) = (&set, dialect.reset_schema()) {
        // Same as `run_script`: a failed reset shows on the next statement anyway.
        let _ = d.query(db, reset);
    }
    out
}

#[cfg(test)]
mod tests;
