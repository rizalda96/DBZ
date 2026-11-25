//! The `Driver` strategy and its factory.

use super::mysql::MySqlDriver;
use super::postgres::PgDriver;
use super::sample::{SampleDriver, SharedSample};
use super::DbError;
use crate::domain::{Cell, ConnConfig, DbKind, FieldDef, ResultSet, RoutineInfo, Row, TableInfo};
use crate::sql::Dialect;

/// One open connection. Implementations own their client and are used from a single
/// worker thread.
pub trait Driver: Send {
    /// Run one or more statements in `db` and return every result set.
    fn query(&mut self, db: &str, sql: &str) -> Result<Vec<ResultSet>, DbError>;
    fn databases(&mut self) -> Result<Vec<String>, DbError>;
    fn tables(&mut self, db: &str) -> Result<Vec<TableInfo>, DbError>;
    fn routines(&mut self, db: &str) -> Result<Vec<RoutineInfo>, DbError>;
    fn columns(&mut self, db: &str, table: &str) -> Result<Vec<FieldDef>, DbError>;
    fn ddl(&mut self, db: &str, table: &str, is_view: bool) -> Result<String, DbError>;
    fn users(&mut self) -> Result<ResultSet, DbError>;
    fn dialect(&self) -> &'static dyn Dialect;

    /// Statements a script run should skip because this server can't run them.
    fn ignores(&self, _stmt: &str) -> bool {
        false
    }

    /// The first result set of `sql` (empty when there is none).
    fn first_set(&mut self, db: &str, sql: &str) -> Result<ResultSet, DbError> {
        Ok(self.query(db, sql)?.into_iter().next().unwrap_or_default())
    }

    fn rows(&mut self, db: &str, sql: &str) -> Result<Vec<Row>, DbError> {
        Ok(self.first_set(db, sql)?.rows)
    }
}

/// Opens a connection for `cfg`. This is the only place that chooses a driver.
pub fn open_driver(cfg: &ConnConfig, sample: &SharedSample) -> Result<Box<dyn Driver>, DbError> {
    match cfg.kind {
        DbKind::Sample => Ok(Box::new(SampleDriver::new(sample.clone()))),
        DbKind::MySql | DbKind::MariaDb => Ok(Box::new(MySqlDriver::connect(cfg)?)),
        DbKind::PostgreSql => Ok(Box::new(PgDriver::connect(cfg)?)),
    }
}

/// Text of a catalogue cell, empty for NULL.
pub(super) fn text(c: &Cell) -> String {
    c.clone().unwrap_or_default()
}

/// A catalogue cell parsed as a number.
pub(super) fn number<T: std::str::FromStr>(c: &Cell) -> Option<T> {
    c.as_ref().and_then(|v| v.parse().ok())
}
