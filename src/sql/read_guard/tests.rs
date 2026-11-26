use super::{check_read, ReadKind};
use crate::sql::{CapRows, Dialect, MssqlDialect, MySqlDialect, PgDialect};

fn ok(d: &dyn Dialect, sql: &str, k: ReadKind) -> String {
    check_read(d, sql, k).expect("allowed")
}

fn no(d: &dyn Dialect, sql: &str, k: ReadKind) {
    assert!(check_read(d, sql, k).is_err(), "{sql}");
}

#[test]
fn select_allows_read_statements() {
    assert_eq!(ok(&PgDialect, "  select * from t; ", ReadKind::Select), "select * from t");
    for s in ["WITH x AS (SELECT 1) SELECT * FROM x", "SHOW TABLES", "DESCRIBE t", "VALUES (1)", "TABLE t", "SELECT x FROM (SELECT 1 AS x) s"] {
        ok(&MySqlDialect, s, ReadKind::Select);
    }
}

#[test]
fn select_rejects_writes_and_side_effects() {
    let bad = [
        "DELETE FROM t",
        "UPDATE t SET a=1",
        "SELECT 1; SELECT 2",
        "SELECT * INTO t2 FROM t",
        "SELECT * FROM t FOR UPDATE",
        "SELECT pg_sleep(10)",
        "SELECT NEXT VALUE FOR s",
        "SELECT * FROM OPENQUERY(srv, 'x')",
        "SELECT GET_LOCK('a', 1)",
        "EXPLAIN SELECT 1",
        "EXEC sp_who",
    ];
    for s in bad {
        no(&MssqlDialect, s, ReadKind::Select);
    }
}

#[test]
fn words_inside_strings_and_comments_still_count() {
    no(&PgDialect, "SELECT 'delete' AS a", ReadKind::Select);
    no(&PgDialect, "SELECT a /* drop */ FROM t", ReadKind::Select);
}

#[test]
fn mssql_batch_without_semicolons_is_rejected() {
    no(&MssqlDialect, "SELECT 1 COMMIT DROP TABLE t", ReadKind::Select);
    no(&MssqlDialect, "SELECT 1 EXEC xp_cmdshell 'whoami'", ReadKind::Select);
    no(&MssqlDialect, "UPDATE t SET a = 1 COMMIT DROP TABLE t", ReadKind::Explain);
}

#[test]
fn pg_dollar_quotes_cannot_hide_statements() {
    no(&PgDialect, "SELECT $$'$$ AS a) s; COMMIT; DELETE FROM t; SELECT * FROM (SELECT $$'$$", ReadKind::Select);
    no(&PgDialect, "SELECT $$'$$ AS a FROM t", ReadKind::Select);
    no(&PgDialect, "SELECT $$'$$; DELETE FROM t; SELECT $$'$$", ReadKind::Explain);
}

#[test]
fn mysql_backslash_escapes_cannot_hide_statements() {
    no(&MySqlDialect, "SELECT \"\\\"\" ; COMMIT; DELETE FROM t; SELECT \"x\"", ReadKind::Select);
    no(&MySqlDialect, "SELECT 'a\\'b' AS x", ReadKind::Select);
}

#[test]
fn hash_is_kept_as_written() {
    assert_eq!(ok(&PgDialect, "SELECT 5 # 3", ReadKind::Select), "SELECT 5 # 3");
}

#[test]
fn explain_allows_dml_but_never_analyze() {
    for s in ["UPDATE t SET a = 1 WHERE id = 2", "DELETE FROM t", "INSERT INTO t VALUES (1)", "SELECT 1", "MERGE INTO t USING s ON 1=1 WHEN MATCHED THEN DELETE"] {
        ok(&PgDialect, s, ReadKind::Explain);
    }
    for s in ["ANALYZE SELECT 1", "EXPLAIN ANALYZE SELECT 1", "DROP TABLE t", "CREATE VIEW v AS SELECT 1", "SELECT pg_sleep(1)", "SELECT 1; SELECT 2"] {
        no(&PgDialect, s, ReadKind::Explain);
    }
}

#[test]
fn pg_caps_by_wrapping() {
    assert_eq!(PgDialect.cap_rows("SELECT * FROM t;", 201), CapRows::Wrapped("SELECT * FROM (SELECT * FROM t) AS dbz_q LIMIT 201".into()));
    assert_eq!(PgDialect.cap_rows("SHOW search_path", 201), CapRows::Wrapped("SHOW search_path".into()));
}

#[test]
fn mysql_and_mssql_cap_with_session_settings() {
    assert_eq!(MySqlDialect.cap_rows("SELECT 1", 201), CapRows::Session { on: "SET SESSION sql_select_limit = 201".into(), off: "SET SESSION sql_select_limit = DEFAULT" });
    assert_eq!(MssqlDialect.cap_rows("SELECT 1", 201), CapRows::Session { on: "SET ROWCOUNT 201".into(), off: "SET ROWCOUNT 0" });
}

#[test]
fn read_only_session_begin_and_timeouts() {
    assert_eq!(PgDialect.read_only_session(), Some("SET SESSION CHARACTERISTICS AS TRANSACTION READ ONLY"));
    assert_eq!(MySqlDialect.read_only_session(), Some("SET SESSION TRANSACTION READ ONLY"));
    assert_eq!(MssqlDialect.read_only_session(), None);
    assert_eq!(PgDialect.read_only_begin(), "BEGIN READ ONLY");
    assert_eq!(MySqlDialect.read_only_begin(), "START TRANSACTION READ ONLY");
    assert_eq!(MssqlDialect.read_only_begin(), "BEGIN TRANSACTION");
    assert_eq!(PgDialect.statement_timeout(30).as_deref(), Some("SET statement_timeout = '30s'"));
    assert_eq!(MySqlDialect.statement_timeout(30).as_deref(), Some("SET SESSION max_execution_time = 30000"));
    assert_eq!(MssqlDialect.statement_timeout(30), None);
}
