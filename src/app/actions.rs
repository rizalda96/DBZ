//! Messages from the UI to the app. Widgets push these; `Dbz::do_act` handles them.

use super::state::{ObjMode, Sel};
use crate::domain::{ConnConfig, DbKind};
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
    SelectFolder(u64, String, ObjMode),
    ToggleFolder(u64, String, ObjMode),
    SetMode(ObjMode),
    LoadDdl(ObjRef, bool),
    // tables and routines
    OpenTable(ObjRef, bool),
    DesignTable(u64, String, Option<String>),
    DropTables(u64, String, Vec<String>, bool),
    Truncate(ObjRef),
    Rename(ObjRef),
    DropRoutine(ObjRef, String),
    // queries
    NewQuery(u64, String, String),
    OpenSavedQuery(ObjRef),
    DeleteQuery(ObjRef),
    SaveQuery(u64),
    WriteQuery(u64, String),
    OpenSqlFile,
    // files
    Export(ObjRef),
    Import(u64, String, Option<String>),
    Dump(u64, String, bool),
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
    DropTables(u64, String, Vec<String>, bool),
    DropDb(u64, String),
    DeleteConn(u64),
    Truncate(ObjRef),
    DeleteQuery(ObjRef),
    DeleteBackup(ObjRef),
    RestoreBackup(ObjRef),
    DropRoutine(ObjRef, String),
    CloseDirtyTab(usize),
}
