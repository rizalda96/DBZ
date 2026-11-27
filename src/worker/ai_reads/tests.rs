use super::ai_read;
use crate::db::{DbError, Driver};
use crate::domain::{DbRead, FieldDef, ResultSet, RoutineInfo, TableInfo, TableParts};
use crate::sql::{Dialect, MySqlDialect, PgDialect};

/// Records every statement; fails the one equal to `fail`.
struct FakeDriver {
    dialect: &'static dyn Dialect,
    log: Vec<String>,
    fail: Option<&'static str>,
}

impl FakeDriver {
    fn new(dialect: &'static dyn Dialect) -> Self {
        FakeDriver { dialect, log: vec![], fail: None }
    }
}

impl Driver for FakeDriver {
    fn query(&mut self, _db: &str, sql: &str) -> Result<Vec<ResultSet>, DbError> {
        self.log.push(sql.to_string());
        if self.fail == Some(sql) {
            return Err(DbError::Query("boom".into()));
        }
        Ok(vec![ResultSet::default()])
    }
    fn databases(&mut self) -> Result<Vec<String>, DbError> {
        Ok(vec![])
    }
    fn tables(&mut self, _db: &str) -> Result<Vec<TableInfo>, DbError> {
        Ok(vec![])
    }
    fn routines(&mut self, _db: &str) -> Result<Vec<RoutineInfo>, DbError> {
        Ok(vec![])
    }
    fn columns(&mut self, _db: &str, _table: &str) -> Result<Vec<FieldDef>, DbError> {
        Ok(vec![])
    }
    fn ddl(&mut self, _db: &str, _table: &str, _is_view: bool) -> Result<String, DbError> {
        Ok(String::new())
    }
    fn routine_ddl(&mut self, _db: &str, _routine: &RoutineInfo) -> Result<String, DbError> {
        Ok(String::new())
    }
    fn table_parts(&mut self, _db: &str, _table: &str) -> Result<TableParts, DbError> {
        Ok(TableParts::default())
    }
    fn ref_columns(&mut self, _db: &str, _schema: &str, _table: &str) -> Result<Vec<String>, DbError> {
        Ok(vec![])
    }
    fn users(&mut self) -> Result<ResultSet, DbError> {
        Ok(ResultSet::default())
    }
    fn dialect(&self) -> &'static dyn Dialect {
        self.dialect
    }
}

static MYSQL: MySqlDialect = MySqlDialect;
static PG: PgDialect = PgDialect;

#[test]
fn select_runs_in_read_only_txn_with_cap() {
    let mut d = FakeDriver::new(&MYSQL);
    ai_read(&mut d, "shop", "", DbRead::Select { sql: "SELECT * FROM t;".into() }).expect("ok");
    assert_eq!(
        d.log,
        [
            "SET SESSION TRANSACTION READ ONLY",
            "SET SESSION max_execution_time = 30000",
            "START TRANSACTION READ ONLY",
            "SET SESSION sql_select_limit = 201",
            "SELECT * FROM t",
            "SET SESSION sql_select_limit = DEFAULT",
            "ROLLBACK"
        ]
    );
}

#[test]
fn rejected_select_sends_nothing() {
    let mut d = FakeDriver::new(&PG);
    let e = ai_read(&mut d, "shop", "", DbRead::Select { sql: "DELETE FROM t".into() }).expect_err("rejected");
    assert!(matches!(e, DbError::Invalid(_)));
    assert!(d.log.is_empty());
}

#[test]
fn rollback_runs_even_when_the_query_fails() {
    let mut d = FakeDriver::new(&MYSQL);
    d.fail = Some("SELECT broken");
    assert!(ai_read(&mut d, "shop", "", DbRead::Select { sql: "SELECT broken".into() }).is_err());
    assert_eq!(d.log.last().map(String::as_str), Some("ROLLBACK"));
}

#[test]
fn explain_never_opens_a_transaction() {
    let mut d = FakeDriver::new(&PG);
    ai_read(&mut d, "shop", "", DbRead::Explain { sql: "UPDATE t SET a = 1".into() }).expect("ok");
    assert_eq!(d.log, ["SET SESSION CHARACTERISTICS AS TRANSACTION READ ONLY", "EXPLAIN UPDATE t SET a = 1"]);
}

#[test]
fn pg_select_wraps_and_uses_schema() {
    let mut d = FakeDriver::new(&PG);
    ai_read(&mut d, "shop", "sales", DbRead::Select { sql: "SELECT 1".into() }).expect("ok");
    assert_eq!(
        d.log,
        [
            "SET SESSION CHARACTERISTICS AS TRANSACTION READ ONLY",
            "SET statement_timeout = '30s'",
            "BEGIN READ ONLY",
            "SET search_path TO \"sales\", public",
            "SELECT * FROM (SELECT 1) AS dbz_q LIMIT 201",
            "RESET search_path",
            "ROLLBACK"
        ]
    );
}
