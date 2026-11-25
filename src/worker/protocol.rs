//! Messages between the UI thread and a connection's worker thread.

use std::time::Duration;

use crate::db::DbError;
use crate::domain::{FieldDef, IndexChoices, ResultSet, RoutineInfo, TableInfo, TableParts};
use crate::transfer::{ExportOptions, ImportOptions};

/// Rows of one page of a table, plus how it was fetched.
#[derive(Clone, Debug)]
pub struct PageQuery {
    pub table: String,
    pub filter: String,
    pub order: Option<(String, bool)>,
    pub limit: usize,
    pub offset: usize,
}

pub enum Req {
    Connect,
    Databases,
    Tables(String),
    Routines(String),
    Schemas(String),
    Columns { db: String, table: String },
    TableParts { db: String, table: String },
    /// Schemas, collations and operator classes for the index fields editor.
    IndexChoices(String),
    /// Columns of a table a foreign key references, for the Referenced Fields drop-down.
    RefColumns { db: String, schema: String, table: String },
    Ddl { db: String, table: String, is_view: bool },
    Query { tab: u64, db: String, sql: String },
    TableData { tab: u64, db: String, page: PageQuery },
    Exec { tab: u64, db: String, stmts: Vec<String>, purpose: Purpose },
    Export { db: String, opts: ExportOptions },
    Import { db: String, opts: ImportOptions },
    Dump { db: String, path: String, data: bool },
    RunFile { db: String, path: String, continue_on_error: bool },
    Users,
}

/// Why statements were sent, so the reply can be routed.
#[derive(Clone, Debug, PartialEq)]
pub enum Purpose {
    SaveData,
    SaveDesign { new_name: String },
    Refresh,
    DropTables,
    Other(String),
}

/// A table page: its columns, the rows and the total row count when known.
pub type TablePage = (Vec<FieldDef>, ResultSet, Option<u64>);

pub enum Resp {
    Connected { version: String },
    ConnectFailed(DbError),
    Databases(Result<Vec<String>, DbError>),
    Tables(String, Result<Vec<TableInfo>, DbError>),
    Routines(String, Result<Vec<RoutineInfo>, DbError>),
    Schemas(String, Result<Vec<String>, DbError>),
    Columns { db: String, table: String, result: Result<Vec<FieldDef>, DbError> },
    TableParts { db: String, table: String, result: Result<TableParts, DbError> },
    IndexChoices(String, Result<IndexChoices, DbError>),
    RefColumns { db: String, schema: String, table: String, result: Result<Vec<String>, DbError> },
    Ddl { db: String, table: String, result: Result<String, DbError> },
    Query { tab: u64, results: Vec<ResultSet>, error: Option<(String, DbError)>, elapsed: Duration },
    TableData { tab: u64, result: Result<TablePage, DbError>, sql: String, elapsed: Duration },
    Exec { tab: u64, db: String, result: Result<u64, DbError>, purpose: Purpose },
    Task(Result<String, DbError>),
    Users(Result<ResultSet, DbError>),
}

/// A reply tagged with the connection id it came from.
pub type Reply = (u64, Resp);
