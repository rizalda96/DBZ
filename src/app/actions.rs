//! Messages from the UI to the app. Widgets push these; `Dbz::do_act` handles them.

use super::state::{ObjMode, Sel};
use crate::domain::{ConnConfig, DbKind, RelationKind};
use crate::transfer::{ArchivePart, ConnSource};
use crate::ui::theme::ThemePref;
use crate::worker::Req;

/// A table or view in a database of a connection.
#[derive(Clone, Debug, PartialEq)]
pub struct ObjRef {
    pub cid: u64,
    pub db: String,
    pub name: String,
}

impl ObjRef {
    pub fn new(cid: u64, db: &str, name: &str) -> Self {
        ObjRef { cid, db: db.to_string(), name: name.to_string() }
    }
}

/// A folder in the connection tree (Tables, Views…), of a database or one of its schemas.
#[derive(Clone, Debug, PartialEq)]
pub struct FolderRef {
    pub cid: u64,
    pub db: String,
    pub schema: Option<String>,
    pub mode: ObjMode,
}

pub enum Act {
    // connections
    Send(u64, Req),
    EnsureOpen(u64),
    OpenConn(u64),
    CloseConn(u64),
    ToggleConn(u64),
    EditConn(u64),
    NewConn(DbKind),
    DupConn(u64),
    DeleteConn(u64),
    RefreshConn(u64),
    TestConn(ConnConfig),
    SaveConn { editing: Option<u64>, cfg: ConnConfig },
    CloseConnDialog,
    /// File › Import Connections: opens its dialog.
    ShowImportConns,
    /// Reads another tool's connections from a file (blank: its default place) and adds them.
    ImportConns(ConnSource, String),
    LoadUsers(u64),
    // databases
    ToggleDb(u64, String),
    OpenDb(u64, String),
    CloseDb(u64, String),
    RefreshDb(u64, String),
    NewDb(u64),
    DropDb(u64, String),
    // selection and navigation
    Select(Sel),
    SelectFolder(FolderRef),
    ToggleFolder(FolderRef),
    ToggleSchema(u64, String, String),
    SetMode(ObjMode),
    LoadDdl(ObjRef, bool),
    // tables and routines
    OpenTable(ObjRef, bool),
    DesignTable(u64, String, Option<String>),
    /// Opens a view's query in a Design View tab.
    DesignView(ObjRef),
    /// Saves a Design View tab's query as its view's definition.
    RunView(u64),
    DropTables(u64, String, Vec<String>, RelationKind),
    Truncate(ObjRef),
    Rename(ObjRef),
    DropRoutine(ObjRef, String),
    // queries
    NewQuery(u64, String, String),
    /// A new query that starts with a SELECT of the table, in the server's syntax.
    QueryTable(ObjRef),
    OpenSavedQuery(ObjRef),
    DeleteQuery(ObjRef),
    SaveQuery(u64),
    WriteQuery(u64, String),
    /// Materialized view designer: open a new one in (connection, database).
    NewMatView(u64, String),
    /// Save the designer in this tab (asks for a name the first time).
    SaveMatView(u64),
    /// The name typed in its Save dialog.
    NameMatView(u64, String),
    OpenSqlFile,
    // files
    Export(ObjRef),
    Import(u64, String, Option<String>),
    Dump(u64, String, bool),
    /// Dump in the server's own format (PostgreSQL `.dump`).
    DumpArchive(u64, String, ArchivePart),
    /// Pick an archive dump and restore it into the database.
    RestoreArchive(u64, String),
    RunFile(u64, String),
    NewBackup(u64, String),
    RestoreBackup(ObjRef),
    DeleteBackup(ObjRef),
    // window
    Confirmed(Confirm),
    CloseTab(usize),
    Activate(usize),
    Alert(String, String),
    Status(String),
    SetTheme(ThemePref),
    ShowAbout,
}

/// Destructive actions waiting for the user's OK.
pub enum Confirm {
    DropTables(u64, String, Vec<String>, RelationKind),
    DropDb(u64, String),
    DeleteConn(u64),
    Truncate(ObjRef),
    DeleteQuery(ObjRef),
    DeleteBackup(ObjRef),
    RestoreBackup(ObjRef),
    /// (connection, database, archive file).
    RestoreArchive(u64, String, String),
    DropRoutine(ObjRef, String),
    CloseDirtyTab(usize),
}
