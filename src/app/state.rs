//! What the app knows: connections, their databases, the selection and open tabs.
//! The UI reads this; only `app` changes it.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::mpsc::Sender;

use super::actions::Act;
use super::tabs::Tab;
use crate::db::DbError;
use crate::domain::{ConnConfig, DbKind, ResultSet, RoutineInfo, TableInfo};
use crate::sql::{dialect_for, Dialect};
use crate::ui::icons::Icon;
use crate::worker::Req;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum ObjMode {
    Tables,
    Views,
    Functions,
    Queries,
    Backups,
    Users,
}

impl ObjMode {
    pub fn label(self) -> &'static str {
        match self {
            ObjMode::Tables => "Tables",
            ObjMode::Views => "Views",
            ObjMode::Functions => "Functions",
            ObjMode::Queries => "Queries",
            ObjMode::Backups => "Backups",
            ObjMode::Users => "Users",
        }
    }

    pub fn icon(self) -> Icon {
        match self {
            ObjMode::Tables => Icon::Table,
            ObjMode::Views => Icon::View,
            ObjMode::Functions => Icon::Function,
            ObjMode::Queries => Icon::Query,
            ObjMode::Backups => Icon::Backup,
            ObjMode::Users => Icon::User,
        }
    }
}

#[derive(Clone, PartialEq, Debug)]
pub enum ConnStatus {
    Closed,
    Opening,
    Open,
    Failed(String),
}

/// A saved query or backup file.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct SavedFile {
    pub name: String,
    pub size: u64,
    pub modified: String,
    pub path: PathBuf,
    /// Start of the file, shown in the info pane (queries only).
    pub preview: String,
}

pub struct DbState {
    pub name: String,
    pub expanded: bool,
    pub loaded: bool,
    pub loading: bool,
    pub tables: Vec<TableInfo>,
    pub routines: Vec<RoutineInfo>,
    pub error: Option<String>,
    pub open_folders: HashSet<ObjMode>,
    pub queries: Vec<SavedFile>,
    pub backups: Vec<SavedFile>,
}

impl DbState {
    pub fn new(name: String) -> Self {
        let open_folders = HashSet::from([ObjMode::Tables]);
        DbState { name, expanded: false, loaded: false, loading: false, tables: vec![], routines: vec![], error: None, open_folders, queries: vec![], backups: vec![] }
    }

    pub fn files(&self, mode: ObjMode) -> &[SavedFile] {
        if mode == ObjMode::Queries {
            &self.queries
        } else {
            &self.backups
        }
    }
}

pub struct ConnState {
    pub cfg: ConnConfig,
    pub status: ConnStatus,
    pub tx: Option<Sender<Req>>,
    pub version: String,
    pub dbs: Vec<DbState>,
    pub expanded: bool,
    pub users: Option<Result<ResultSet, DbError>>,
}

impl ConnState {
    pub fn new(cfg: ConnConfig) -> Self {
        ConnState { cfg, status: ConnStatus::Closed, tx: None, version: String::new(), dbs: vec![], expanded: false, users: None }
    }

    pub fn is_open(&self) -> bool {
        self.tx.is_some()
    }

    pub fn db(&self, name: &str) -> Option<&DbState> {
        self.dbs.iter().find(|d| d.name == name)
    }

    pub fn db_mut(&mut self, name: &str) -> Option<&mut DbState> {
        self.dbs.iter_mut().find(|d| d.name == name)
    }
}

#[derive(Clone, Default, PartialEq, Debug)]
pub struct Sel {
    pub cid: Option<u64>,
    pub db: Option<String>,
    pub obj: Option<String>,
}

impl Sel {
    pub fn conn(cid: u64) -> Self {
        Sel { cid: Some(cid), db: None, obj: None }
    }

    pub fn db(cid: u64, db: impl Into<String>) -> Self {
        Sel { cid: Some(cid), db: Some(db.into()), obj: None }
    }

    /// Connection and database, when both are selected.
    pub fn conn_db(&self) -> Option<(u64, String)> {
        Some((self.cid?, self.db.clone()?))
    }
}

/// (connection, database, table) of a cached DDL text.
pub type DdlKey = (u64, String, String);

/// Connections, selection and tabs.
pub struct Workspace {
    pub conns: Vec<ConnState>,
    pub sel: Sel,
    pub tabs: Vec<Tab>,
    /// 0 = the Objects tab, i = tabs[i - 1].
    pub active: usize,
    pub mode: ObjMode,
    /// DDL texts for the info pane; `Ok("")` while loading.
    pub ddl_cache: HashMap<DdlKey, Result<String, DbError>>,
}

impl Workspace {
    pub fn conn(&self, cid: u64) -> Option<&ConnState> {
        self.conns.iter().find(|c| c.cfg.id == cid)
    }

    pub fn conn_mut(&mut self, cid: u64) -> Option<&mut ConnState> {
        self.conns.iter_mut().find(|c| c.cfg.id == cid)
    }

    pub fn db_mut(&mut self, cid: u64, db: &str) -> Option<&mut DbState> {
        self.conn_mut(cid).and_then(|c| c.db_mut(db))
    }

    pub fn kind_of(&self, cid: u64) -> DbKind {
        self.conn(cid).map(|c| c.cfg.kind).unwrap_or(DbKind::Sample)
    }

    pub fn dialect_of(&self, cid: u64) -> &'static dyn Dialect {
        dialect_for(self.kind_of(cid))
    }

    /// The selected connection, or the first one, for "New Query".
    pub fn query_target(&self) -> Option<u64> {
        self.sel.cid.or(self.conns.first().map(|c| c.cfg.id))
    }

    pub fn tab_mut(&mut self, id: u64) -> Option<&mut Tab> {
        self.tabs.iter_mut().find(|t| t.id() == id)
    }

    /// True when the Objects tab shows `mode`.
    pub fn showing(&self, mode: ObjMode) -> bool {
        self.mode == mode && self.active == 0
    }

    /// Acts that refresh whatever is selected (F5 and View › Refresh).
    pub fn refresh_acts(&self) -> Vec<Act> {
        let mut acts = vec![];
        match (self.sel.cid, self.sel.db.clone()) {
            (Some(cid), Some(db)) => acts.push(Act::RefreshDb(cid, db)),
            (Some(cid), None) => acts.push(Act::RefreshConn(cid)),
            _ => {}
        }
        if let (ObjMode::Users, Some(cid)) = (self.mode, self.sel.cid) {
            acts.push(Act::LoadUsers(cid));
        }
        acts
    }
}

/// Pane settings the user changes directly in the UI.
pub struct PaneState {
    pub obj_filter: String,
    pub tree_filter: String,
    pub show_info: bool,
    pub info_ddl: bool,
}

impl Default for PaneState {
    fn default() -> Self {
        PaneState { obj_filter: String::new(), tree_filter: String::new(), show_info: true, info_ddl: false }
    }
}
