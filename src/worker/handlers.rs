//! Turns one request into one reply.

use std::time::Instant;

use super::protocol::{PageQuery, Purpose, Req, Resp, TablePage};
use crate::db::{DbError, Driver};
use crate::domain::{first_cell, ConnConfig, ResultSet};
use crate::sql::split_statements;
use crate::transfer;

pub(super) fn handle(d: &mut dyn Driver, cfg: &ConnConfig, req: Req) -> Resp {
    match req {
        Req::Connect => Resp::Connected { version: version(d, cfg) },
        Req::Databases => Resp::Databases(d.databases()),
        Req::Tables(db) => {
            let r = d.tables(&db);
            Resp::Tables(db, r)
        }
        Req::Routines(db) => {
            let r = d.routines(&db);
            Resp::Routines(db, r)
        }
        Req::Schemas(db) => {
            let r = d.schemas(&db);
            Resp::Schemas(db, r)
        }
        Req::Columns { db, table } => {
            let result = d.columns(&db, &table);
            Resp::Columns { db, table, result }
        }
        Req::TableParts { db, table } => {
            let result = d.table_parts(&db, &table);
            Resp::TableParts { db, table, result }
        }
        Req::IndexChoices(db) => {
            let r = d.index_choices(&db);
            Resp::IndexChoices(db, r)
        }
        Req::RefColumns { db, schema, table } => {
            let result = d.ref_columns(&db, &schema, &table);
            Resp::RefColumns { db, schema, table, result }
        }
        Req::Ddl { db, table, is_view } => {
            let result = d.ddl(&db, &table, is_view);
            Resp::Ddl { db, table, result }
        }
        Req::Query { tab, db, schema, sql } => run_script(d, tab, (&db, &schema), &sql),
        Req::TableData { tab, db, page } => table_page(d, tab, &db, &page),
        Req::Exec { tab, db, stmts, purpose } => {
            let result = exec(d, &db, &stmts, &purpose);
            Resp::Exec { tab, db, result, purpose }
        }
        Req::Export { db, opts } => Resp::Task(transfer::export(d, &db, &opts)),
        Req::Import { db, opts } => Resp::Task(transfer::import(d, &db, &opts)),
        Req::Dump { db, path, data } => Resp::Task(transfer::dump(d, cfg, &db, &path, data)),
        Req::RunFile { db, path, continue_on_error } => Resp::Task(transfer::run_file(d, &db, &path, continue_on_error)),
        Req::Users => Resp::Users(d.users()),
    }
}

fn version(d: &mut dyn Driver, cfg: &ConnConfig) -> String {
    d.query(&cfg.initial_db, "SELECT VERSION()").ok().and_then(|sets| first_cell(&sets)).unwrap_or_default()
}

/// Runs an editor script in `db`, with unqualified names resolving in `schema` first
/// (when the server has schemas and one is chosen).
fn run_script(d: &mut dyn Driver, tab: u64, (db, schema): (&str, &str), sql: &str) -> Resp {
    let start = Instant::now();
    let dialect = d.dialect();
    let use_schema = dialect.use_schema(schema);
    if let Some(set) = &use_schema {
        if let Err(e) = d.query(db, set) {
            return Resp::Query { tab, results: vec![], error: Some((set.clone(), e)), elapsed: start.elapsed() };
        }
    }
    let (results, error) = run_statements(d, db, sql);
    if let (Some(_), Some(reset)) = (&use_schema, dialect.reset_schema()) {
        // The script already has its answer. A reset can only fail inside a
        // transaction the script left aborted, and the user sees that on the next run.
        let _ = d.query(db, reset);
    }
    Resp::Query { tab, results, error, elapsed: start.elapsed() }
}

/// Runs each statement of a script, stopping at the first error.
fn run_statements(d: &mut dyn Driver, db: &str, sql: &str) -> (Vec<ResultSet>, Option<(String, DbError)>) {
    let mut results = vec![];
    for stmt in split_statements(sql) {
        match d.query(db, &stmt) {
            Ok(sets) => results.extend(label_sets(sets, &stmt)),
            Err(e) => return (results, Some((stmt, e))),
        }
    }
    (results, None)
}

/// Tags every set with its statement; a statement with no sets still gets one.
fn label_sets(mut sets: Vec<ResultSet>, stmt: &str) -> Vec<ResultSet> {
    if sets.is_empty() {
        sets.push(ResultSet::default());
    }
    for s in &mut sets {
        s.statement = stmt.to_string();
    }
    sets
}

fn table_page(d: &mut dyn Driver, tab: u64, db: &str, page: &PageQuery) -> Resp {
    let start = Instant::now();
    let dialect = d.dialect();
    let qt = dialect.quote_ident(&page.table);
    let filter = page.filter.trim();
    let where_ = if filter.is_empty() { String::new() } else { format!(" WHERE {}", filter) };
    let mut sql = format!("SELECT * FROM {}{}", qt, where_);
    if let Some((col, desc)) = &page.order {
        sql.push_str(&format!(" ORDER BY {} {}", dialect.quote_ident(col), if *desc { "DESC" } else { "ASC" }));
    }
    sql.push_str(&format!(" LIMIT {} OFFSET {}", page.limit, page.offset));
    let count_sql = format!("SELECT COUNT(*) FROM {}{}", qt, where_);
    let result = fetch_page(d, db, &page.table, &sql, &count_sql);
    Resp::TableData { tab, result, sql, elapsed: start.elapsed() }
}

fn fetch_page(d: &mut dyn Driver, db: &str, table: &str, sql: &str, count_sql: &str) -> Result<TablePage, DbError> {
    let fields = d.columns(db, table)?;
    let rs = d.first_set(db, sql)?;
    let total = d.query(db, count_sql).ok().and_then(|sets| first_cell(&sets)).and_then(|s| s.parse().ok());
    Ok((fields, rs, total))
}

/// Runs statements in order; grid saves of several rows run in one transaction.
fn exec(d: &mut dyn Driver, db: &str, stmts: &[String], purpose: &Purpose) -> Result<u64, DbError> {
    // A server may reject BEGIN/COMMIT/ROLLBACK (e.g. tables without transactions).
    // That must not block the save, so those replies are deliberately ignored and the
    // statements' own results decide success.
    let transactional = *purpose == Purpose::SaveData && stmts.len() > 1;
    if transactional {
        let begin = d.dialect().begin_transaction();
        let _ = d.query(db, begin);
    }
    let result = exec_all(d, db, stmts);
    if transactional {
        let _ = d.query(db, if result.is_err() { "ROLLBACK" } else { "COMMIT" });
    }
    result
}

fn exec_all(d: &mut dyn Driver, db: &str, stmts: &[String]) -> Result<u64, DbError> {
    let mut total = 0;
    for s in stmts {
        let sets = d.query(db, s).map_err(|e| DbError::Query(format!("{}\n\nStatement:\n{}", e, s)))?;
        total += sets.iter().map(|x| x.affected).sum::<u64>();
    }
    Ok(total)
}
