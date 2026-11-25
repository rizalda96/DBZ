//! `Driver` for the in-memory sample server.

use std::sync::MutexGuard;

use super::{SampleServer, SharedSample};
use crate::db::{DbError, Driver};
use crate::domain::{FieldDef, ResultSet, RoutineInfo, TableInfo, TableParts};
use crate::sql::{dialect_for, split_statements, Dialect};

pub struct SampleDriver {
    server: SharedSample,
}

impl SampleDriver {
    pub fn new(server: SharedSample) -> Self {
        SampleDriver { server }
    }

    fn lock(&self) -> Result<MutexGuard<'_, SampleServer>, DbError> {
        self.server.lock().map_err(|_| DbError::query("The sample database is unavailable after an earlier error"))
    }
}

impl Driver for SampleDriver {
    fn query(&mut self, db: &str, sql: &str) -> Result<Vec<ResultSet>, DbError> {
        let mut srv = self.lock()?;
        split_statements(sql).iter().map(|st| srv.exec(db, st)).collect()
    }

    fn databases(&mut self) -> Result<Vec<String>, DbError> {
        Ok(self.lock()?.databases())
    }

    fn tables(&mut self, db: &str) -> Result<Vec<TableInfo>, DbError> {
        self.lock()?.tables(db)
    }

    fn routines(&mut self, db: &str) -> Result<Vec<RoutineInfo>, DbError> {
        Ok(self.lock()?.routines(db))
    }

    fn columns(&mut self, db: &str, table: &str) -> Result<Vec<FieldDef>, DbError> {
        self.lock()?.columns(db, table)
    }

    fn ddl(&mut self, db: &str, table: &str, _is_view: bool) -> Result<String, DbError> {
        self.lock()?.ddl(db, table)
    }

    fn table_parts(&mut self, db: &str, table: &str) -> Result<TableParts, DbError> {
        Ok(self.lock()?.table(db, table)?.parts.clone().loaded())
    }

    fn users(&mut self) -> Result<ResultSet, DbError> {
        Ok(ResultSet {
            columns: vec!["User".into(), "Host".into(), "Note".into()],
            rows: vec![vec![Some("sample".into()), Some("localhost".into()), Some("The built-in sample database has a single user".into())]],
            has_rows: true,
            ..Default::default()
        })
    }

    fn dialect(&self) -> &'static dyn Dialect {
        dialect_for(crate::domain::DbKind::Sample)
    }

    /// The sample engine has no session settings, so `SET ...` in a dump is skipped.
    fn ignores(&self, stmt: &str) -> bool {
        stmt.to_uppercase().starts_with("SET ")
    }
}
