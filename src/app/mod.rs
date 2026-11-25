//! The application: owns all state, turns `Act`s from the UI into state changes and
//! worker requests, and applies worker replies. Handlers are grouped by concern in
//! separate `impl Dbz` blocks.

mod actions;
mod confirm;
mod conn_actions;
mod db_actions;
mod dialog_state;
mod dispatch;
mod exec_replies;
mod file_actions;
mod frame;
mod persistence;
mod replies;
mod screenshot;
mod state;
mod tab_actions;
mod tabs;

pub use actions::{Act, Confirm, ObjRef};
pub use dialog_state::{ConnDialog, Dialog, DialogState};
pub use state::{ConnState, ConnStatus, DbState, ObjMode, PaneState, Sel, Workspace};
pub use tabs::Tab;

use std::collections::HashMap;
use std::sync::mpsc::{Receiver, Sender};
use std::sync::{Arc, Mutex};

use eframe::egui;

use crate::db::{sample_server, SharedSample};
use crate::domain::{ConnConfig, DbKind};
use crate::ui::style::setup_style;
use crate::worker::{Reply, ReplyTo, Req};
use persistence::{Settings, Store};
use screenshot::Screenshot;

/// Channels and shared resources for talking to workers.
struct Runtime {
    reply: ReplyTo,
    rx: Receiver<Reply>,
    sample: SharedSample,
    /// The worker of a running "Test Connection".
    test_tx: Option<(u64, Sender<Req>)>,
}

/// Counters for new ids.
struct Ids {
    next_conn: u64,
    next_tab: u64,
    query_counter: u32,
}

pub struct Dbz {
    ws: Workspace,
    panes: PaneState,
    dialogs: DialogState,
    rt: Runtime,
    store: Store,
    settings: Settings,
    status: String,
    ids: Ids,
    shot: Option<Screenshot>,
    /// Database (and table) to open once a connection lists its databases.
    auto_open: Option<(u64, String, Option<String>)>,
}

impl Dbz {
    pub fn new(cc: &eframe::CreationContext<'_>, screenshot: Option<String>) -> Self {
        // Screenshot runs use defaults and never touch the user's files.
        let store = Store::new(screenshot.is_none());
        let settings = if screenshot.is_some() { Settings::from_env() } else { store.load_settings() };
        setup_style(&cc.egui_ctx, settings.theme);
        let mut cfgs = store.load_connections();
        if cfgs.is_empty() {
            cfgs.push(ConnConfig::sample(1));
        }
        let next_conn = cfgs.iter().map(|c| c.id).max().unwrap_or(0) + 1;
        let sample_id = cfgs.iter().find(|c| c.kind == DbKind::Sample).map(|c| c.id);
        let (tx, rx) = std::sync::mpsc::channel();
        let mut app = Dbz {
            ws: Workspace { conns: cfgs.into_iter().map(ConnState::new).collect(), sel: Sel::default(), tabs: vec![], active: 0, mode: ObjMode::Tables, ddl_cache: HashMap::new() },
            panes: PaneState::default(),
            dialogs: DialogState::default(),
            rt: Runtime { reply: ReplyTo { tx, ctx: cc.egui_ctx.clone() }, rx, sample: Arc::new(Mutex::new(sample_server())), test_tx: None },
            store,
            settings,
            status: "Ready".into(),
            ids: Ids { next_conn, next_tab: 1, query_counter: 0 },
            shot: screenshot.map(Screenshot::new),
            auto_open: None,
        };
        if let Some(id) = sample_id {
            app.auto_open = Some((id, "shop".into(), Some("customers".into())));
            app.do_act(Act::OpenConn(id));
        }
        app
    }

    fn ctx(&self) -> &egui::Context {
        &self.rt.reply.ctx
    }

    fn new_tab_id(&mut self) -> u64 {
        self.ids.next_tab += 1;
        self.ids.next_tab
    }

    fn alert(&mut self, title: impl Into<String>, msg: impl Into<String>) {
        self.dialogs.alert = Some((title.into(), msg.into()));
    }

    fn confirm(&mut self, title: impl Into<String>, msg: impl Into<String>, action: Confirm) {
        self.dialogs.dialog = Some(Dialog::Confirm { title: title.into(), msg: msg.into(), action });
    }
}
