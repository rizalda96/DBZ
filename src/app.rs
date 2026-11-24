//! App state, actions and worker replies. The window pieces live in `panes.rs`.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, Sender};
use std::sync::{Arc, Mutex};

use eframe::egui::{self, vec2, Color32, FontId, Stroke, Ui};

use crate::backend::{self, ExportFormat, Purpose, Reply, Req, Resp};
use crate::demo::{self, DemoServer};
use crate::icons::Icon;
use crate::model::*;
use crate::views::{self, *};

// ---------------------------------------------------------------- state

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

pub struct DbState {
    pub name: String,
    pub expanded: bool,
    pub loaded: bool,
    pub loading: bool,
    pub tables: Vec<TableInfo>,
    pub routines: Vec<RoutineInfo>,
    pub error: Option<String>,
    pub open_folders: HashSet<ObjMode>,
    pub queries: Vec<(String, u64, String)>,
    pub backups: Vec<(String, u64, String)>,
}

impl DbState {
    fn new(name: String) -> Self {
        let mut open_folders = HashSet::new();
        open_folders.insert(ObjMode::Tables);
        DbState { name, expanded: false, loaded: false, loading: false, tables: vec![], routines: vec![], error: None, open_folders, queries: vec![], backups: vec![] }
    }
}

pub struct ConnState {
    pub cfg: ConnConfig,
    pub status: ConnStatus,
    pub tx: Option<Sender<Req>>,
    pub version: String,
    pub dbs: Vec<DbState>,
    pub expanded: bool,
    pub users: Option<Result<ResultSet, String>>,
}

impl ConnState {
    pub fn new(cfg: ConnConfig) -> Self {
        ConnState { cfg, status: ConnStatus::Closed, tx: None, version: String::new(), dbs: vec![], expanded: false, users: None }
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

pub enum Tab {
    Query(QueryTab),
    Data(DataTab),
    Design(DesignTab),
}

impl Tab {
    pub fn id(&self) -> u64 {
        match self {
            Tab::Query(t) => t.id,
            Tab::Data(t) => t.id,
            Tab::Design(t) => t.id,
        }
    }
    pub fn cid(&self) -> u64 {
        match self {
            Tab::Query(t) => t.cid,
            Tab::Data(t) => t.cid,
            Tab::Design(t) => t.cid,
        }
    }
    pub fn title(&self, conns: &[ConnState]) -> String {
        let cname = |cid: u64| conns.iter().find(|c| c.cfg.id == cid).map(|c| c.cfg.name.clone()).unwrap_or_default();
        match self {
            Tab::Query(t) => format!("{}{} @{} ({})", if t.dirty { "* " } else { "" }, t.title, t.db, cname(t.cid)),
            Tab::Data(t) => format!("{}{} @{} ({})", if t.dirty() { "* " } else { "" }, t.table, t.db, cname(t.cid)),
            Tab::Design(t) => format!("{} @{} ({})", if t.table.is_some() { t.name.clone() } else { "Untitled".into() }, t.db, cname(t.cid)),
        }
    }
    pub fn icon(&self) -> Icon {
        match self {
            Tab::Query(_) => Icon::Query,
            Tab::Data(t) => {
                if t.is_view {
                    Icon::View
                } else {
                    Icon::Table
                }
            }
            Tab::Design(_) => Icon::Design,
        }
    }
}

pub enum Act {
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
    ToggleDb(u64, String),
    OpenDb(u64, String),
    CloseDb(u64, String),
    RefreshDb(u64, String),
    NewDb(u64),
    DropDb(u64, String),
    Select(Sel),
    SelectFolder(u64, String, ObjMode),
    ToggleFolder(u64, String, ObjMode),
    SetMode(ObjMode),
    OpenTable(u64, String, String, bool),
    DesignTable(u64, String, Option<String>),
    DropTables(u64, String, Vec<String>, bool),
    Truncate(u64, String, String),
    Rename(u64, String, String),
    NewQuery(u64, String, String),
    OpenSavedQuery(u64, String, String),
    DeleteQuery(u64, String, String),
    SaveQuery(u64),
    Export(u64, String, String),
    Import(u64, String, Option<String>),
    Dump(u64, String, bool),
    RunFile(u64, String),
    NewBackup(u64, String),
    RestoreBackup(u64, String, String),
    DeleteBackup(u64, String, String),
    DropRoutine(u64, String, String, String),
    CloseTab(usize),
    Activate(usize),
    Alert(String, String),
    Status(String),
    LoadUsers(u64),
    ShowAbout,
}

pub enum Confirm {
    DropTables(u64, String, Vec<String>, bool),
    DropDb(u64, String),
    DeleteConn(u64),
    Truncate(u64, String, String),
    DeleteQuery(u64, String, String),
    DeleteBackup(u64, String, String),
    RestoreBackup(u64, String, String),
    DropRoutine(u64, String, String, String),
    CloseDirtyTab(usize),
}

pub enum Dialog {
    Confirm { title: String, msg: String, action: Confirm },
    NewDb { cid: u64, name: String, charset: String },
    Export(ExportState),
    Import { cid: u64, db: String, table: String, path: String, header: bool, delim: char, create: bool },
    RunFile { cid: u64, db: String, path: String, cont: bool },
    SaveQuery { tab: u64, name: String },
    Rename { cid: u64, db: String, table: String, new: String },
    About,
}

pub struct ConnDialog {
    pub editing: Option<u64>,
    pub cfg: ConnConfig,
    pub page: usize,
    pub test: Option<Result<String, String>>,
    pub test_id: Option<u64>,
}

pub struct DbStudio {
    pub conns: Vec<ConnState>,
    pub sel: Sel,
    pub tabs: Vec<Tab>,
    pub active: usize,
    pub mode: ObjMode,
    pub obj_filter: String,
    pub tree_filter: String,
    pub show_info: bool,
    pub info_ddl: bool,
    pub ddl_cache: HashMap<(u64, String, String), Result<String, String>>,
    pub tx: Sender<Reply>,
    pub rx: Receiver<Reply>,
    pub sample: Arc<Mutex<DemoServer>>,
    pub next_conn: u64,
    pub next_tab: u64,
    pub status: String,
    pub alert: Option<(String, String)>,
    pub dialog: Option<Dialog>,
    pub conn_dialog: Option<ConnDialog>,
    pub test_tx: Option<(u64, Sender<Req>)>,
    pub screenshot: Option<String>,
    pub frames: u32,
    pub shot_sent: bool,
    pub data_dir: PathBuf,
    pub auto_open: Option<(u64, String, Option<String>)>,
    pub ctx: egui::Context,
    pub query_counter: u32,
}

fn data_dir() -> PathBuf {
    std::env::var_os("APPDATA").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(".")).join("DBStudio")
}

pub fn sanitize(s: &str) -> String {
    s.chars().map(|c| if c.is_alphanumeric() || c == '-' || c == '_' || c == '.' || c == ' ' { c } else { '_' }).collect()
}

pub fn sample_cfg(id: u64) -> ConnConfig {
    ConnConfig { id, name: "Sample Data".into(), kind: DbKind::Sample, host: "built-in".into(), port: 0, user: "sample".into(), password: String::new(), save_password: true, initial_db: String::new(), connect_timeout: 0 }
}

fn setup_style(ctx: &egui::Context) {
    ctx.set_theme(egui::Theme::Light);
    ctx.global_style_mut(|s| {
        s.spacing.item_spacing = vec2(6.0, 4.0);
        s.spacing.button_padding = vec2(8.0, 3.0);
        s.spacing.interact_size.y = 22.0;
        let v = &mut s.visuals;
        v.panel_fill = Color32::from_rgb(0xfa, 0xfa, 0xfb);
        v.window_fill = Color32::WHITE;
        v.extreme_bg_color = Color32::WHITE;
        v.faint_bg_color = Color32::from_rgb(0xf6, 0xf8, 0xfb);
        v.selection.bg_fill = SEL_BG;
        v.selection.stroke = Stroke::new(1.0, ACCENT);
        v.hyperlink_color = ACCENT;
        v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, BORDER);
        v.widgets.noninteractive.fg_stroke = Stroke::new(1.0, TEXT);
        v.widgets.inactive.fg_stroke = Stroke::new(1.0, TEXT);
        v.widgets.inactive.weak_bg_fill = Color32::from_rgb(0xf1, 0xf2, 0xf4);
        v.widgets.inactive.bg_stroke = Stroke::new(1.0, Color32::from_rgb(0xd0, 0xd4, 0xda));
        v.widgets.hovered.weak_bg_fill = HOVER_BG;
        v.widgets.hovered.bg_stroke = Stroke::new(1.0, Color32::from_rgb(0x9c, 0xc0, 0xec));
        v.widgets.active.weak_bg_fill = SEL_BG;
        v.striped = true;
        s.text_styles.insert(egui::TextStyle::Body, FontId::proportional(13.5));
        s.text_styles.insert(egui::TextStyle::Button, FontId::proportional(13.5));
        s.text_styles.insert(egui::TextStyle::Monospace, FontId::monospace(13.0));
    });
}

impl DbStudio {
    pub fn new(cc: &eframe::CreationContext<'_>, screenshot: Option<String>) -> Self {
        setup_style(&cc.egui_ctx);
        let (tx, rx) = std::sync::mpsc::channel();
        let dir = data_dir();
        let mut cfgs: Vec<ConnConfig> = vec![];
        if screenshot.is_none() {
            if let Ok(text) = std::fs::read_to_string(dir.join("connections.json")) {
                cfgs = serde_json::from_str(&text).unwrap_or_default();
            }
        }
        if cfgs.is_empty() {
            cfgs.push(sample_cfg(1));
        }
        let next_conn = cfgs.iter().map(|c| c.id).max().unwrap_or(0) + 1;
        let sample_id = cfgs.iter().find(|c| c.kind == DbKind::Sample).map(|c| c.id);
        let mut app = DbStudio {
            conns: cfgs.into_iter().map(ConnState::new).collect(),
            sel: Sel::default(),
            tabs: vec![],
            active: 0,
            mode: ObjMode::Tables,
            obj_filter: String::new(),
            tree_filter: String::new(),
            show_info: true,
            info_ddl: false,
            ddl_cache: HashMap::new(),
            tx,
            rx,
            sample: Arc::new(Mutex::new(demo::sample_server())),
            next_conn,
            next_tab: 1,
            status: "Ready".into(),
            alert: None,
            dialog: None,
            conn_dialog: None,
            test_tx: None,
            screenshot,
            frames: 0,
            shot_sent: false,
            data_dir: dir,
            auto_open: None,
            ctx: cc.egui_ctx.clone(),
            query_counter: 0,
        };
        if let Some(id) = sample_id {
            app.auto_open = Some((id, "shop".into(), Some("customers".into())));
            app.do_act(Act::OpenConn(id));
        }
        app
    }

    pub fn conn(&self, cid: u64) -> Option<&ConnState> {
        self.conns.iter().find(|c| c.cfg.id == cid)
    }
    pub fn conn_mut(&mut self, cid: u64) -> Option<&mut ConnState> {
        self.conns.iter_mut().find(|c| c.cfg.id == cid)
    }

    pub fn save_config(&self) {
        if self.screenshot.is_some() {
            return;
        }
        let cfgs: Vec<ConnConfig> = self
            .conns
            .iter()
            .map(|c| {
                let mut cfg = c.cfg.clone();
                if !cfg.save_password {
                    cfg.password.clear();
                }
                cfg
            })
            .collect();
        let _ = std::fs::create_dir_all(&self.data_dir);
        if let Ok(text) = serde_json::to_string_pretty(&cfgs) {
            let _ = std::fs::write(self.data_dir.join("connections.json"), text);
        }
    }

    pub fn folder(&self, kind: &str, cid: u64, db: &str) -> PathBuf {
        self.data_dir.join(kind).join(cid.to_string()).join(sanitize(db))
    }

    pub fn scan_files(&mut self, cid: u64, db: &str) {
        let list = |dir: PathBuf| -> Vec<(String, u64, String)> {
            let mut v = vec![];
            if let Ok(rd) = std::fs::read_dir(dir) {
                for e in rd.flatten() {
                    let p = e.path();
                    if p.extension().map(|x| x == "sql").unwrap_or(false) {
                        let meta = e.metadata().ok();
                        let size = meta.as_ref().map(|m| m.len()).unwrap_or(0);
                        let modified = meta
                            .and_then(|m| m.modified().ok())
                            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                            .map(|d| crate::panes::fmt_time(d.as_secs() as i64))
                            .unwrap_or_default();
                        v.push((p.file_stem().unwrap_or_default().to_string_lossy().to_string(), size, modified));
                    }
                }
            }
            v.sort();
            v
        };
        let q = list(self.folder("queries", cid, db));
        let b = list(self.folder("backups", cid, db));
        if let Some(d) = self.conn_mut(cid).and_then(|c| c.db_mut(db)) {
            d.queries = q;
            d.backups = b;
        }
    }

    fn new_tab_id(&mut self) -> u64 {
        self.next_tab += 1;
        self.next_tab
    }

    fn send(&mut self, cid: u64, req: Req) {
        self.ensure_open(cid);
        let mut failed = false;
        if let Some(c) = self.conn_mut(cid) {
            if let Some(tx) = &c.tx {
                if tx.send(req).is_err() {
                    failed = true;
                }
            }
        }
        if failed {
            if let Some(c) = self.conn_mut(cid) {
                c.tx = None;
                c.status = ConnStatus::Closed;
            }
            self.status = "The connection was closed. Please try again.".into();
        }
    }

    fn ensure_open(&mut self, cid: u64) {
        let need = self.conn(cid).map(|c| c.tx.is_none()).unwrap_or(false);
        if need {
            self.do_act(Act::OpenConn(cid));
        }
    }

    fn kind_of(&self, cid: u64) -> DbKind {
        self.conn(cid).map(|c| c.cfg.kind).unwrap_or(DbKind::Sample)
    }

    fn confirm(&mut self, title: impl Into<String>, msg: impl Into<String>, action: Confirm) {
        self.dialog = Some(Dialog::Confirm { title: title.into(), msg: msg.into(), action });
    }

    // ------------------------------------------------------------ actions

    pub fn do_act(&mut self, act: Act) {
        match act {
            Act::Send(cid, req) => self.send(cid, req),
            Act::EnsureOpen(cid) => self.ensure_open(cid),
            Act::OpenConn(cid) => {
                let sample = self.sample.clone();
                let tx = self.tx.clone();
                let ctx = self.ctx.clone();
                let mut msg = None;
                if let Some(c) = self.conn_mut(cid) {
                    if c.tx.is_some() {
                        c.expanded = true;
                        return;
                    }
                    let wtx = backend::spawn(c.cfg.clone(), sample, tx, ctx);
                    let _ = wtx.send(Req::Connect);
                    let _ = wtx.send(Req::Databases);
                    c.tx = Some(wtx);
                    c.status = ConnStatus::Opening;
                    c.expanded = true;
                    msg = Some(format!("Connecting to {}…", c.cfg.name));
                }
                if let Some(m) = msg {
                    self.status = m;
                }
                if self.sel.cid != Some(cid) {
                    self.sel = Sel { cid: Some(cid), db: None, obj: None };
                }
            }
            Act::CloseConn(cid) => {
                if let Some(c) = self.conn_mut(cid) {
                    c.tx = None;
                    c.status = ConnStatus::Closed;
                    c.dbs.clear();
                    c.expanded = false;
                    c.users = None;
                }
                self.tabs.retain(|t| t.cid() != cid);
                self.active = self.active.min(self.tabs.len());
                if self.sel.cid == Some(cid) {
                    self.sel = Sel { cid: Some(cid), db: None, obj: None };
                }
            }
            Act::ToggleConn(cid) => {
                let (open, exp) = self.conn(cid).map(|c| (c.tx.is_some(), c.expanded)).unwrap_or((false, false));
                if !open {
                    self.do_act(Act::OpenConn(cid));
                } else if let Some(c) = self.conn_mut(cid) {
                    c.expanded = !exp;
                }
            }
            Act::EditConn(cid) => {
                if let Some(c) = self.conn(cid) {
                    self.conn_dialog = Some(ConnDialog { editing: Some(cid), cfg: c.cfg.clone(), page: 0, test: None, test_id: None });
                }
            }
            Act::NewConn(kind) => {
                let id = self.next_conn;
                let cfg = if kind == DbKind::Sample {
                    sample_cfg(id)
                } else {
                    ConnConfig {
                        id,
                        name: String::new(),
                        kind,
                        host: "localhost".into(),
                        port: kind.default_port(),
                        user: kind.default_user().into(),
                        password: String::new(),
                        save_password: true,
                        initial_db: String::new(),
                        connect_timeout: 10,
                    }
                };
                self.conn_dialog = Some(ConnDialog { editing: None, cfg, page: 0, test: None, test_id: None });
            }
            Act::DupConn(cid) => {
                if let Some(c) = self.conn(cid) {
                    let mut cfg = c.cfg.clone();
                    cfg.id = self.next_conn;
                    cfg.name = format!("{} (copy)", cfg.name);
                    self.next_conn += 1;
                    self.conns.push(ConnState::new(cfg));
                    self.save_config();
                }
            }
            Act::DeleteConn(cid) => {
                let name = self.conn(cid).map(|c| c.cfg.name.clone()).unwrap_or_default();
                self.confirm("Delete Connection", format!("Are you sure you want to delete the connection \"{}\"?", name), Confirm::DeleteConn(cid));
            }
            Act::RefreshConn(cid) => {
                if self.conn(cid).map(|c| c.tx.is_some()).unwrap_or(false) {
                    self.send(cid, Req::Databases);
                }
            }
            Act::ToggleDb(cid, db) => {
                let exp = self.conn(cid).and_then(|c| c.db(&db)).map(|d| d.expanded).unwrap_or(false);
                if exp {
                    if let Some(d) = self.conn_mut(cid).and_then(|c| c.db_mut(&db)) {
                        d.expanded = false;
                    }
                } else {
                    self.do_act(Act::OpenDb(cid, db));
                }
            }
            Act::OpenDb(cid, db) => {
                let mut load = false;
                if let Some(d) = self.conn_mut(cid).and_then(|c| c.db_mut(&db)) {
                    d.expanded = true;
                    if !d.loaded && !d.loading {
                        d.loading = true;
                        load = true;
                    }
                }
                if load {
                    self.send(cid, Req::Tables(db.clone()));
                    self.send(cid, Req::Routines(db.clone()));
                    self.scan_files(cid, &db);
                }
                if let Some(c) = self.conn_mut(cid) {
                    c.expanded = true;
                }
                self.sel = Sel { cid: Some(cid), db: Some(db), obj: None };
                if self.mode == ObjMode::Users {
                    self.mode = ObjMode::Tables;
                }
            }
            Act::CloseDb(cid, db) => {
                if let Some(d) = self.conn_mut(cid).and_then(|c| c.db_mut(&db)) {
                    d.expanded = false;
                    d.loaded = false;
                    d.tables.clear();
                    d.routines.clear();
                }
            }
            Act::RefreshDb(cid, db) => {
                if let Some(d) = self.conn_mut(cid).and_then(|c| c.db_mut(&db)) {
                    d.loading = true;
                }
                self.send(cid, Req::Tables(db.clone()));
                self.send(cid, Req::Routines(db.clone()));
                self.scan_files(cid, &db);
                self.ddl_cache.retain(|k, _| !(k.0 == cid && k.1 == db));
            }
            Act::NewDb(cid) => {
                let pg = self.kind_of(cid).is_pg();
                self.dialog = Some(Dialog::NewDb { cid, name: String::new(), charset: if pg { "UTF8".into() } else { "utf8mb4".into() } });
            }
            Act::DropDb(cid, db) => {
                self.confirm("Delete Database", format!("Are you sure you want to delete the database \"{}\"? All of its tables and data will be lost.", db), Confirm::DropDb(cid, db));
            }
            Act::Select(sel) => self.sel = sel,
            Act::SelectFolder(cid, db, mode) => {
                self.mode = mode;
                self.active = 0;
                self.sel = Sel { cid: Some(cid), db: Some(db), obj: None };
            }
            Act::ToggleFolder(cid, db, mode) => {
                if let Some(d) = self.conn_mut(cid).and_then(|c| c.db_mut(&db)) {
                    if !d.open_folders.remove(&mode) {
                        d.open_folders.insert(mode);
                    }
                }
            }
            Act::SetMode(m) => {
                if self.mode != m {
                    self.sel.obj = None;
                }
                self.mode = m;
                self.active = 0;
                if m == ObjMode::Users {
                    if let Some(cid) = self.sel.cid {
                        self.do_act(Act::LoadUsers(cid));
                    }
                }
                if let (Some(cid), Some(db)) = (self.sel.cid, self.sel.db.clone()) {
                    if m == ObjMode::Queries || m == ObjMode::Backups {
                        self.scan_files(cid, &db);
                    }
                }
            }
            Act::LoadUsers(cid) => {
                if let Some(c) = self.conn_mut(cid) {
                    c.users = None;
                }
                self.send(cid, Req::Users);
            }
            Act::OpenTable(cid, db, table, is_view) => {
                if let Some(i) = self.tabs.iter().position(|t| matches!(t, Tab::Data(d) if d.cid == cid && d.db == db && d.table == table)) {
                    self.active = i + 1;
                    return;
                }
                let kind = self.kind_of(cid);
                let id = self.new_tab_id();
                let mut t = DataTab {
                    id,
                    cid,
                    db,
                    table,
                    is_view,
                    kind,
                    fields: vec![],
                    columns: vec![],
                    rows: vec![],
                    sel: None,
                    editing: None,
                    filter: String::new(),
                    filter_applied: String::new(),
                    show_filter: false,
                    order: None,
                    limit: 1000,
                    offset: 0,
                    total: None,
                    loading: true,
                    saving: false,
                    last_sql: String::new(),
                    error: None,
                    elapsed: None,
                    scroll_to: None,
                };
                let req = t.reload_req();
                self.tabs.push(Tab::Data(t));
                self.active = self.tabs.len();
                self.send(cid, req);
            }
            Act::DesignTable(cid, db, table) => {
                if let Some(tn) = &table {
                    if let Some(i) = self.tabs.iter().position(|t| matches!(t, Tab::Design(d) if d.cid == cid && d.db == db && d.table.as_ref() == Some(tn))) {
                        self.active = i + 1;
                        return;
                    }
                }
                let kind = self.kind_of(cid);
                let id = self.new_tab_id();
                let comment = table.as_ref().and_then(|tn| self.conn(cid).and_then(|c| c.db(&db)).and_then(|d| d.tables.iter().find(|t| &t.name == tn)).map(|t| t.comment.clone())).unwrap_or_default();
                let fields = if table.is_none() {
                    vec![FieldDef { name: "id".into(), ty: if kind.is_pg() { "integer".into() } else { "int".into() }, not_null: true, key: true, auto_inc: true, ..Default::default() }]
                } else {
                    vec![]
                };
                self.tabs.push(Tab::Design(DesignTab {
                    id,
                    cid,
                    db: db.clone(),
                    kind,
                    name: table.clone().unwrap_or_default(),
                    loading: table.is_some(),
                    table: table.clone(),
                    fields,
                    orig: vec![],
                    comment: comment.clone(),
                    orig_comment: comment,
                    sub: 0,
                    sel: Some(0),
                    saving: false,
                    error: None,
                }));
                self.active = self.tabs.len();
                if let Some(tn) = table {
                    self.send(cid, Req::Columns { db, table: tn });
                }
            }
            Act::DropTables(cid, db, names, view) => {
                let what = if view { "view" } else { "table" };
                let msg = if names.len() == 1 { format!("Are you sure you want to delete the {} \"{}\"?", what, names[0]) } else { format!("Are you sure you want to delete these {} {}s?", names.len(), what) };
                self.confirm(format!("Delete {}", if view { "View" } else { "Table" }), msg, Confirm::DropTables(cid, db, names, view));
            }
            Act::Truncate(cid, db, t) => {
                self.confirm("Truncate Table", format!("Are you sure you want to remove every record from \"{}\"? This can't be undone.", t), Confirm::Truncate(cid, db, t));
            }
            Act::Rename(cid, db, t) => {
                self.dialog = Some(Dialog::Rename { cid, db, new: t.clone(), table: t });
            }
            Act::NewQuery(cid, db, sql) => {
                let kind = self.kind_of(cid);
                let db = if db.is_empty() { self.conn(cid).and_then(|c| c.dbs.iter().find(|d| d.expanded).or(c.dbs.first())).map(|d| d.name.clone()).unwrap_or_default() } else { db };
                self.query_counter += 1;
                let id = self.new_tab_id();
                self.tabs.push(Tab::Query(QueryTab {
                    id,
                    cid,
                    db,
                    title: "Untitled".into(),
                    sql,
                    saved_name: None,
                    results: vec![],
                    error: None,
                    running: false,
                    elapsed: None,
                    view: 0,
                    sel_cell: None,
                    dirty: false,
                    selection: None,
                    kind,
                }));
                self.active = self.tabs.len();
                self.ensure_open(cid);
            }
            Act::OpenSavedQuery(cid, db, name) => {
                if let Some(i) = self.tabs.iter().position(|t| matches!(t, Tab::Query(q) if q.cid == cid && q.db == db && q.saved_name.as_deref() == Some(name.as_str()))) {
                    self.active = i + 1;
                    return;
                }
                let path = self.folder("queries", cid, &db).join(format!("{}.sql", name));
                match std::fs::read_to_string(&path) {
                    Ok(sql) => {
                        self.do_act(Act::NewQuery(cid, db, sql));
                        if let Some(Tab::Query(q)) = self.tabs.last_mut() {
                            q.title = name.clone();
                            q.saved_name = Some(name);
                        }
                    }
                    Err(e) => self.alert = Some(("Open Query".into(), format!("Could not read {}: {}", path.display(), e))),
                }
            }
            Act::DeleteQuery(cid, db, name) => {
                self.confirm("Delete Query", format!("Delete the saved query \"{}\"?", name), Confirm::DeleteQuery(cid, db, name));
            }
            Act::SaveQuery(tab) => {
                let Some(Tab::Query(q)) = self.tabs.iter().find(|t| t.id() == tab) else { return };
                match q.saved_name.clone() {
                    Some(name) => self.write_query(tab, name),
                    None => {
                        let n = q.title.clone();
                        self.dialog = Some(Dialog::SaveQuery { tab, name: if n == "Untitled" { format!("Query {}", self.query_counter) } else { n } });
                    }
                }
            }
            Act::Export(cid, db, table) => {
                let path = crate::panes::default_dir().join(format!("{}.csv", sanitize(&table))).to_string_lossy().to_string();
                self.dialog = Some(Dialog::Export(ExportState { cid, db, table, format: ExportFormat::Csv, path, header: true }));
            }
            Act::Import(cid, db, table) => {
                self.dialog = Some(Dialog::Import { cid, db, create: table.is_none(), table: table.unwrap_or_default(), path: String::new(), header: true, delim: ',' });
            }
            Act::Dump(cid, db, data) => {
                let file = rfd::FileDialog::new().set_title("Dump SQL File").add_filter("SQL file", &["sql"]).set_file_name(format!("{}.sql", sanitize(&db))).save_file();
                if let Some(p) = file {
                    self.status = format!("Dumping {}…", db);
                    self.send(cid, Req::Dump { db, path: p.to_string_lossy().to_string(), data });
                }
            }
            Act::RunFile(cid, db) => {
                self.dialog = Some(Dialog::RunFile { cid, db, path: String::new(), cont: false });
            }
            Act::NewBackup(cid, db) => {
                let dir = self.folder("backups", cid, &db);
                if let Err(e) = std::fs::create_dir_all(&dir) {
                    self.alert = Some(("Backup".into(), e.to_string()));
                    return;
                }
                let stamp: String = demo::now_string().chars().filter(|c| c.is_ascii_digit()).collect();
                let path = dir.join(format!("{}.sql", stamp));
                self.status = format!("Backing up {}…", db);
                self.send(cid, Req::Dump { db, path: path.to_string_lossy().to_string(), data: true });
            }
            Act::RestoreBackup(cid, db, name) => {
                self.confirm("Restore Backup", format!("Restore backup \"{}\" into \"{}\"? Tables in the backup will be replaced.", name, db), Confirm::RestoreBackup(cid, db, name));
            }
            Act::DeleteBackup(cid, db, name) => {
                self.confirm("Delete Backup", format!("Delete the backup \"{}\"?", name), Confirm::DeleteBackup(cid, db, name));
            }
            Act::DropRoutine(cid, db, name, kind) => {
                self.confirm(format!("Delete {}", kind.to_lowercase()), format!("Are you sure you want to delete the {} \"{}\"?", kind.to_lowercase(), name), Confirm::DropRoutine(cid, db, name, kind));
            }
            Act::CloseTab(i) => {
                if i == 0 || i > self.tabs.len() {
                    return;
                }
                let dirty = match &self.tabs[i - 1] {
                    Tab::Data(d) => d.dirty(),
                    Tab::Query(q) => q.dirty && !q.sql.trim().is_empty(),
                    Tab::Design(d) => !d.statements().is_empty(),
                };
                if dirty {
                    self.confirm("Close Tab", "This tab has unsaved changes. Close it anyway?", Confirm::CloseDirtyTab(i));
                } else {
                    self.close_tab(i);
                }
            }
            Act::Activate(i) => self.active = i.min(self.tabs.len()),
            Act::Alert(t, m) => self.alert = Some((t, m)),
            Act::Status(s) => self.status = s,
            Act::ShowAbout => self.dialog = Some(Dialog::About),
        }
    }

    pub fn close_tab(&mut self, i: usize) {
        if i == 0 || i > self.tabs.len() {
            return;
        }
        self.tabs.remove(i - 1);
        if self.active >= i {
            self.active = self.active.saturating_sub(1);
        }
        self.active = self.active.min(self.tabs.len());
    }

    pub fn write_query(&mut self, tab: u64, name: String) {
        let Some(Tab::Query(q)) = self.tabs.iter().find(|t| t.id() == tab) else { return };
        let dir = self.folder("queries", q.cid, &q.db);
        let (cid, db, sql) = (q.cid, q.db.clone(), q.sql.clone());
        let clean = sanitize(&name);
        let res = std::fs::create_dir_all(&dir).and_then(|_| std::fs::write(dir.join(format!("{}.sql", clean)), sql));
        match res {
            Ok(_) => {
                if let Some(Tab::Query(q)) = self.tabs.iter_mut().find(|t| t.id() == tab) {
                    q.saved_name = Some(clean.clone());
                    q.title = clean;
                    q.dirty = false;
                }
                self.status = format!("Saved query \"{}\"", name);
                self.scan_files(cid, &db);
            }
            Err(e) => self.alert = Some(("Save Query".into(), e.to_string())),
        }
    }

    pub fn run_confirm(&mut self, action: Confirm) {
        use crate::sqlgen::quote_ident;
        match action {
            Confirm::DropTables(cid, db, names, view) => {
                let pg = self.kind_of(cid).is_pg();
                let stmts = names.iter().map(|n| format!("DROP {} {}", if view { "VIEW" } else { "TABLE" }, quote_ident(pg, n))).collect();
                self.send(cid, Req::Exec { tab: 0, db: db.clone(), stmts, purpose: Purpose::DropTables });
                self.tabs.retain(|t| !matches!(t, Tab::Data(d) if d.cid == cid && d.db == db && names.contains(&d.table)));
                self.active = self.active.min(self.tabs.len());
                self.sel.obj = None;
            }
            Confirm::DropDb(cid, db) => {
                let pg = self.kind_of(cid).is_pg();
                self.send(cid, Req::Exec { tab: 0, db: String::new(), stmts: vec![format!("DROP DATABASE {}", quote_ident(pg, &db))], purpose: Purpose::Other("database".into()) });
                if self.sel.db.as_deref() == Some(db.as_str()) {
                    self.sel = Sel { cid: Some(cid), db: None, obj: None };
                }
            }
            Confirm::DeleteConn(cid) => {
                self.do_act(Act::CloseConn(cid));
                self.conns.retain(|c| c.cfg.id != cid);
                if self.sel.cid == Some(cid) {
                    self.sel = Sel::default();
                }
                self.save_config();
            }
            Confirm::Truncate(cid, db, t) => {
                let pg = self.kind_of(cid).is_pg();
                self.send(cid, Req::Exec { tab: 0, db, stmts: vec![format!("TRUNCATE TABLE {}", quote_ident(pg, &t))], purpose: Purpose::Refresh });
            }
            Confirm::DeleteQuery(cid, db, name) => {
                let _ = std::fs::remove_file(self.folder("queries", cid, &db).join(format!("{}.sql", name)));
                self.scan_files(cid, &db);
                self.sel.obj = None;
            }
            Confirm::DeleteBackup(cid, db, name) => {
                let _ = std::fs::remove_file(self.folder("backups", cid, &db).join(format!("{}.sql", name)));
                self.scan_files(cid, &db);
                self.sel.obj = None;
            }
            Confirm::RestoreBackup(cid, db, name) => {
                let path = self.folder("backups", cid, &db).join(format!("{}.sql", name));
                self.status = format!("Restoring {}…", name);
                self.send(cid, Req::RunFile { db, path: path.to_string_lossy().to_string(), continue_on_error: true });
            }
            Confirm::DropRoutine(cid, db, name, kind) => {
                let pg = self.kind_of(cid).is_pg();
                let kw = if kind.eq_ignore_ascii_case("procedure") { "PROCEDURE" } else { "FUNCTION" };
                self.send(cid, Req::Exec { tab: 0, db, stmts: vec![format!("DROP {} {}", kw, quote_ident(pg, &name))], purpose: Purpose::Refresh });
            }
            Confirm::CloseDirtyTab(i) => self.close_tab(i),
        }
    }

    // ------------------------------------------------------------ worker replies

    pub fn poll(&mut self) {
        while let Ok((cid, resp)) = self.rx.try_recv() {
            if let Some(d) = &mut self.conn_dialog {
                if d.test_id == Some(cid) {
                    match resp {
                        Resp::Connected { version } => d.test = Some(Ok(if version.is_empty() { "Connection successful".into() } else { format!("Connection successful — server {}", version) })),
                        Resp::ConnectFailed(e) => d.test = Some(Err(e)),
                        _ => {}
                    }
                    self.test_tx = None;
                    continue;
                }
            }
            if cid > u64::MAX / 2 {
                continue; // reply from a finished connection test
            }
            self.handle(cid, resp);
        }
    }

    fn handle(&mut self, cid: u64, resp: Resp) {
        match resp {
            Resp::Connected { version } => {
                let mut msg = None;
                if let Some(c) = self.conn_mut(cid) {
                    c.status = ConnStatus::Open;
                    c.version = version;
                    msg = Some(format!("Connected to {}", c.cfg.name));
                }
                if let Some(m) = msg {
                    self.status = m;
                }
            }
            Resp::ConnectFailed(e) => {
                let name = self.conn(cid).map(|c| c.cfg.name.clone()).unwrap_or_default();
                if let Some(c) = self.conn_mut(cid) {
                    c.status = ConnStatus::Failed(e.clone());
                    c.tx = None;
                    c.expanded = false;
                }
                for t in self.tabs.iter_mut() {
                    match t {
                        Tab::Query(q) if q.cid == cid => q.running = false,
                        Tab::Data(d) if d.cid == cid => {
                            d.loading = false;
                            d.error = Some(e.clone())
                        }
                        Tab::Design(d) if d.cid == cid => {
                            d.loading = false;
                            d.saving = false
                        }
                        _ => {}
                    }
                }
                self.alert = Some((format!("Can't connect to \"{}\"", name), e));
                self.status = "Connection failed".into();
            }
            Resp::Databases(r) => match r {
                Ok(list) => {
                    if let Some(c) = self.conn_mut(cid) {
                        let mut old: Vec<DbState> = std::mem::take(&mut c.dbs);
                        c.dbs = list
                            .into_iter()
                            .map(|n| match old.iter().position(|d| d.name == n) {
                                Some(i) => old.remove(i),
                                None => DbState::new(n),
                            })
                            .collect();
                    }
                    if let Some((acid, db, table)) = self.auto_open.clone() {
                        if acid == cid {
                            self.auto_open = None;
                            if self.conn(cid).and_then(|c| c.db(&db)).is_some() {
                                self.do_act(Act::OpenDb(cid, db));
                                if let Some(t) = table {
                                    self.sel.obj = Some(t);
                                }
                            }
                        }
                    }
                }
                Err(e) => self.alert = Some(("Databases".into(), e)),
            },
            Resp::Tables(db, r) => {
                let mut err = None;
                if let Some(d) = self.conn_mut(cid).and_then(|c| c.db_mut(&db)) {
                    d.loading = false;
                    match r {
                        Ok(t) => {
                            d.tables = t;
                            d.loaded = true;
                            d.error = None;
                        }
                        Err(e) => {
                            d.error = Some(e.clone());
                            err = Some(e);
                        }
                    }
                }
                if let Some(e) = err {
                    self.status = format!("Error loading tables: {}", e);
                }
            }
            Resp::Routines(db, r) => {
                if let Some(d) = self.conn_mut(cid).and_then(|c| c.db_mut(&db)) {
                    d.routines = r.unwrap_or_default();
                }
            }
            Resp::Columns { db, table, result } => {
                for t in self.tabs.iter_mut() {
                    if let Tab::Design(d) = t {
                        if d.cid == cid && d.db == db && d.table.as_deref() == Some(table.as_str()) && d.loading {
                            d.loading = false;
                            match &result {
                                Ok(f) => {
                                    d.fields = f.clone();
                                    d.orig = f.clone();
                                }
                                Err(e) => d.error = Some(e.clone()),
                            }
                        }
                    }
                }
            }
            Resp::Ddl { db, table, result } => {
                self.ddl_cache.insert((cid, db, table), result);
            }
            Resp::Query { tab, results, error, elapsed } => {
                let mut changed_schema = None;
                let mut status = None;
                if let Some(Tab::Query(q)) = self.tabs.iter_mut().find(|t| t.id() == tab) {
                    q.running = false;
                    q.elapsed = Some(elapsed);
                    q.view = if error.is_some() { 0 } else { results.iter().position(|r| r.has_rows).map(|i| i + 1).unwrap_or(0) };
                    q.sel_cell = None;
                    let ddl = results.iter().any(|r| {
                        let u = r.statement.trim_start().to_uppercase();
                        ["CREATE", "DROP", "ALTER", "RENAME", "INSERT", "DELETE", "TRUNCATE", "UPDATE"].iter().any(|k| u.starts_with(k))
                    });
                    if ddl {
                        changed_schema = Some(q.db.clone());
                    }
                    status = Some(match &error {
                        Some((_, e)) => format!("Query failed: {}", e.lines().next().unwrap_or("")),
                        None => format!("Query finished in {:.3}s", elapsed.as_secs_f64()),
                    });
                    q.results = results;
                    q.error = error;
                }
                if let Some(s) = status {
                    self.status = s;
                }
                if let Some(db) = changed_schema {
                    self.do_act(Act::RefreshConn(cid));
                    if self.conn(cid).and_then(|c| c.db(&db)).map(|d| d.loaded).unwrap_or(false) {
                        self.do_act(Act::RefreshDb(cid, db));
                    }
                }
            }
            Resp::TableData { tab, result, sql, elapsed } => {
                if let Some(Tab::Data(d)) = self.tabs.iter_mut().find(|t| t.id() == tab) {
                    d.loading = false;
                    d.elapsed = Some(elapsed);
                    d.last_sql = sql;
                    match result {
                        Ok((fields, rs, total)) => {
                            d.fields = fields;
                            d.columns = rs.columns;
                            d.rows = rs.rows.into_iter().map(|r| RowState { orig: Some(r.clone()), cur: r, deleted: false }).collect();
                            d.total = total;
                            d.error = None;
                            d.editing = None;
                            if let Some((r, _)) = d.sel {
                                if r >= d.rows.len() {
                                    d.sel = None;
                                }
                            }
                        }
                        Err(e) => d.error = Some(e),
                    }
                }
            }
            Resp::Exec { tab, db, result, purpose } => match (&purpose, result) {
                (Purpose::SaveData, Ok(n)) => {
                    let mut req = None;
                    if let Some(Tab::Data(d)) = self.tabs.iter_mut().find(|t| t.id() == tab) {
                        d.saving = false;
                        req = Some(d.reload_req());
                    }
                    if let Some(r) = req {
                        self.send(cid, r);
                    }
                    self.status = format!("Changes saved ({} row(s) affected)", n);
                }
                (Purpose::SaveData, Err(e)) => {
                    if let Some(Tab::Data(d)) = self.tabs.iter_mut().find(|t| t.id() == tab) {
                        d.saving = false;
                    }
                    self.alert = Some(("Couldn't save changes".into(), e));
                }
                (Purpose::SaveDesign { new_name }, Ok(_)) => {
                    if let Some(Tab::Design(d)) = self.tabs.iter_mut().find(|t| t.id() == tab) {
                        d.saving = false;
                        d.table = Some(new_name.clone());
                        d.name = new_name.clone();
                        d.loading = true;
                        d.orig_comment = d.comment.clone();
                    }
                    self.send(cid, Req::Columns { db: db.clone(), table: new_name.clone() });
                    self.status = format!("Table \"{}\" saved", new_name);
                    self.do_act(Act::RefreshDb(cid, db.clone()));
                    let reqs: Vec<Req> = self
                        .tabs
                        .iter_mut()
                        .filter_map(|t| match t {
                            Tab::Data(d) if d.cid == cid && d.db == db && &d.table == new_name => Some(d.reload_req()),
                            _ => None,
                        })
                        .collect();
                    for r in reqs {
                        self.send(cid, r);
                    }
                }
                (Purpose::SaveDesign { .. }, Err(e)) => {
                    if let Some(Tab::Design(d)) = self.tabs.iter_mut().find(|t| t.id() == tab) {
                        d.saving = false;
                        d.error = Some(e.clone());
                    }
                    self.alert = Some(("Couldn't save the table".into(), e));
                }
                (_, Ok(n)) => {
                    if !db.is_empty() {
                        self.do_act(Act::RefreshDb(cid, db.clone()));
                    }
                    self.do_act(Act::RefreshConn(cid));
                    self.status = format!("Done ({} row(s) affected)", n);
                }
                (_, Err(e)) => {
                    self.alert = Some(("Error".into(), e));
                }
            },
            Resp::Task(r) => {
                match r {
                    Ok(msg) => {
                        self.status = msg.clone();
                        self.alert = Some(("Finished".into(), msg));
                    }
                    Err(e) => self.alert = Some(("Error".into(), e)),
                }
                let dbs: Vec<String> = self.conn(cid).map(|c| c.dbs.iter().filter(|d| d.loaded).map(|d| d.name.clone()).collect()).unwrap_or_default();
                for db in dbs {
                    self.do_act(Act::RefreshDb(cid, db));
                }
            }
            Resp::Users(r) => {
                if let Some(c) = self.conn_mut(cid) {
                    c.users = Some(r);
                }
            }
        }
    }

    pub fn refresh_current(&self, acts: &mut Vec<Act>) {
        match (self.sel.cid, self.sel.db.clone()) {
            (Some(cid), Some(db)) => acts.push(Act::RefreshDb(cid, db)),
            (Some(cid), None) => acts.push(Act::RefreshConn(cid)),
            _ => {}
        }
        if self.mode == ObjMode::Users {
            if let Some(cid) = self.sel.cid {
                acts.push(Act::LoadUsers(cid));
            }
        }
    }

    fn shortcuts(&mut self, ctx: &egui::Context, acts: &mut Vec<Act>) {
        let (q, w, f5) = ctx.input_mut(|i| (i.consume_key(egui::Modifiers::COMMAND, egui::Key::Q), i.consume_key(egui::Modifiers::COMMAND, egui::Key::W), i.consume_key(egui::Modifiers::NONE, egui::Key::F5)));
        if q {
            if let Some(cid) = self.sel.cid.or(self.conns.first().map(|c| c.cfg.id)) {
                acts.push(Act::NewQuery(cid, self.sel.db.clone().unwrap_or_default(), String::new()));
            }
        }
        if w && self.active > 0 {
            acts.push(Act::CloseTab(self.active));
        }
        if f5 {
            if self.active > 0 {
                if let Some(Tab::Data(d)) = self.tabs.get_mut(self.active - 1) {
                    let r = d.reload_req();
                    acts.push(Act::Send(d.cid, r));
                    return;
                }
            }
            self.refresh_current(acts);
        }
    }
}

impl eframe::App for DbStudio {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if self.screenshot.is_some() {
            ctx.send_viewport_cmd(egui::ViewportCommand::RequestPaintWhileHidden);
            ctx.request_repaint();
        }
    }

    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        self.poll();
        let ctx = ui.ctx().clone();
        let mut acts: Vec<Act> = vec![];
        if self.alert.is_none() && self.dialog.is_none() && self.conn_dialog.is_none() {
            self.shortcuts(&ctx, &mut acts);
        }
        egui::Panel::top("menu").frame(egui::Frame::new().fill(Color32::WHITE).inner_margin(egui::Margin::symmetric(6, 2))).show(ui, |ui| self.menu_bar(ui, &mut acts));
        egui::Panel::top("ribbon")
            .frame(egui::Frame::new().fill(Color32::from_rgb(0xf4, 0xf5, 0xf7)).inner_margin(egui::Margin::symmetric(8, 4)).stroke(Stroke::new(1.0, BORDER)))
            .show(ui, |ui| self.ribbon(ui, &mut acts));
        egui::Panel::bottom("status")
            .exact_size(26.0)
            .frame(egui::Frame::new().fill(Color32::from_rgb(0xf0, 0xf1, 0xf3)).inner_margin(egui::Margin::symmetric(6, 0)).stroke(Stroke::new(1.0, BORDER)))
            .show(ui, |ui| self.status_bar(ui));
        egui::Panel::left("nav").resizable(true).default_size(260.0).min_size(170.0).frame(egui::Frame::new().fill(Color32::WHITE).stroke(Stroke::new(1.0, BORDER))).show(ui, |ui| self.nav(ui, &mut acts));
        if self.show_info {
            egui::Panel::right("info").resizable(true).default_size(270.0).min_size(180.0).frame(egui::Frame::new().fill(Color32::WHITE).stroke(Stroke::new(1.0, BORDER))).show(ui, |ui| self.info(ui, &mut acts));
        }
        egui::CentralPanel::default().frame(egui::Frame::new().fill(Color32::WHITE)).show(ui, |ui| {
            self.tab_bar(ui, &mut acts);
            if self.active == 0 {
                self.objects(ui, &mut acts);
            } else {
                let i = self.active - 1;
                let conns = &self.conns;
                match &mut self.tabs[i] {
                    Tab::Query(t) => views::query_view(ui, t, conns, &mut acts),
                    Tab::Data(t) => views::data_view(ui, t, &mut acts),
                    Tab::Design(t) => views::design_view(ui, t, &mut acts),
                }
            }
        });
        self.dialogs(&ctx, &mut acts);
        for a in acts {
            self.do_act(a);
        }

        if let Some(path) = self.screenshot.clone() {
            self.frames += 1;
            ctx.request_repaint();
            // Test helper: DBSTUDIO_SHOT_VIEW=data|query|design opens that screen before the picture.
            if self.frames == 20 {
                let cid = self.conns.first().map(|c| c.cfg.id).unwrap_or(1);
                match std::env::var("DBSTUDIO_SHOT_VIEW").unwrap_or_default().as_str() {
                    "data" => self.do_act(Act::OpenTable(cid, "shop".into(), "customers".into(), false)),
                    "design" => self.do_act(Act::DesignTable(cid, "shop".into(), Some("orders".into()))),
                    "query" => {
                        let sql = "-- Orders per status\nSELECT id, customer_id, ordered_at, status, total\nFROM orders\nWHERE total > 50\nORDER BY total DESC\nLIMIT 20;".to_string();
                        self.do_act(Act::NewQuery(cid, "shop".into(), sql.clone()));
                        if let Some(Tab::Query(q)) = self.tabs.last_mut() {
                            q.running = true;
                            let id = q.id;
                            self.do_act(Act::Send(cid, Req::Query { tab: id, db: "shop".into(), sql }));
                        }
                    }
                    _ => {}
                }
            }
            if self.frames >= 120 && !self.shot_sent {
                self.shot_sent = true;
                ctx.request_screenshot(move |img| {
                    let [w, h] = img.size;
                    let mut buf = Vec::with_capacity(w * h * 4);
                    for p in &img.pixels {
                        buf.extend_from_slice(&[p.r(), p.g(), p.b(), 255]);
                    }
                    if let Some(parent) = std::path::Path::new(&path).parent() {
                        if !parent.as_os_str().is_empty() {
                            let _ = std::fs::create_dir_all(parent);
                        }
                    }
                    let code = match image::save_buffer(&path, &buf, w as u32, h as u32, image::ColorType::Rgba8) {
                        Ok(_) => 0,
                        Err(e) => {
                            eprintln!("could not save screenshot: {}", e);
                            1
                        }
                    };
                    std::process::exit(code);
                });
            }
        }
    }
}
