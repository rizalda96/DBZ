//! Messages between the UI thread and a connection's worker thread.

use std::time::Duration;

use crate::db::DbError;
use crate::domain::{DbRead, FieldDef, IndexChoices, ResultSet, RoutineInfo, TableInfo, TableParts};
use crate::transfer::{ArchivePart, ExportOptions, ImportOptions};

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
    /// A view's definition, to open in an editor (Design View).
    ViewSource { db: String, view: String },
    /// A function's or procedure's definition, to open in a query tab.
    RoutineSource { db: String, routine: RoutineInfo },
    /// `schema` is where unqualified names resolve first; empty = the server's default.
    /// `explain` asks for the plans of the statements instead of running them.
    Query { tab: u64, db: String, schema: String, sql: String, explain: bool },
    TableData { tab: u64, db: String, page: PageQuery },
    Exec { tab: u64, db: String, stmts: Vec<String>, purpose: Purpose },
    Export { db: String, opts: ExportOptions },
    Import { db: String, opts: ImportOptions },
    Dump { db: String, path: String, data: bool },
    RunFile { db: String, path: String, continue_on_error: bool },
    /// The server's own dump format (PostgreSQL `.dump`), through its client tools.
    DumpArchive { db: String, path: String, part: ArchivePart },
    RestoreArchive { db: String, path: String },
    Users,
    /// One of the AI's read tools, on the AI's own connection.
    AiRead { db: String, schema: String, read: DbRead },
}

impl Req {
    /// What a long file task is doing, for the "please wait" window; None for the
    /// quick requests. Its reply is always a `Resp::Task`.
    pub fn task_label(&self) -> Option<String> {
        let file = |p: &str| std::path::Path::new(p).file_name().map(|f| f.to_string_lossy().to_string()).unwrap_or_else(|| p.to_string());
        Some(match self {
            Req::Export { opts, .. } => format!("Exporting {} to {}…", opts.table, file(&opts.path)),
            Req::Import { db, opts } => format!("Importing {} into {}…", file(&opts.path), db),
            Req::Dump { db, path, .. } | Req::DumpArchive { db, path, .. } => format!("Dumping {} to {}…", db, file(path)),
            Req::RunFile { db, path, .. } => format!("Running {} on {}…", file(path), db),
            Req::RestoreArchive { db, path } => format!("Restoring {} into {}…", file(path), db),
            _ => return None,
        })
    }
}

/// Why statements were sent, so the reply can be routed.
#[derive(Clone, Debug, PartialEq)]
pub enum Purpose {
    SaveData,
    SaveDesign { new_name: String },
    SaveMatView,
    SaveView,
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
    ViewSource { db: String, view: String, result: Result<String, DbError> },
    RoutineSource { db: String, name: String, result: Result<String, DbError> },
    Query { tab: u64, results: Vec<ResultSet>, error: Option<(String, DbError)>, elapsed: Duration },
    TableData { tab: u64, result: Result<TablePage, DbError>, sql: String, elapsed: Duration },
    Exec { tab: u64, db: String, result: Result<u64, DbError>, purpose: Purpose },
    Task(Result<String, DbError>),
    Users(Result<ResultSet, DbError>),
    /// The tool's answer as text for the AI.
    AiRead(Result<String, DbError>),
}

/// A reply tagged with the connection id it came from.
pub type Reply = (u64, Resp);
