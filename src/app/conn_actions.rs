//! Connections: open/close, edit, test, save, and sending requests to workers.

use super::{ConnDialog, ConnState, ConnStatus, Confirm, Dbz, Sel};
use crate::domain::{ConnConfig, DbKind};
use crate::ui::theme::ThemePref;
use crate::worker::{self, Req};

impl Dbz {
    /// Sends `req` to the connection's worker, opening the connection if needed.
    pub(super) fn send(&mut self, cid: u64, req: Req) {
        self.ensure_open(cid);
        let failed = self.ws.conn(cid).and_then(|c| c.tx.as_ref()).is_some_and(|tx| tx.send(req).is_err());
        if !failed {
            return;
        }
        if let Some(c) = self.ws.conn_mut(cid) {
            c.tx = None;
            c.status = ConnStatus::Closed;
        }
        self.status = "The connection was closed. Please try again.".into();
    }

    pub(super) fn ensure_open(&mut self, cid: u64) {
        if self.ws.conn(cid).is_some_and(|c| !c.is_open()) {
            self.open_conn(cid);
        }
    }

    pub(super) fn open_conn(&mut self, cid: u64) {
        let reply = self.rt.reply.clone();
        let Some(c) = self.ws.conn_mut(cid) else { return };
        c.expanded = true;
        if c.is_open() {
            return;
        }
        let tx = worker::spawn(c.cfg.clone(), reply);
        // A fresh channel cannot be closed yet, so these sends cannot fail.
        let _ = tx.send(Req::Connect);
        let _ = tx.send(Req::Databases);
        c.tx = Some(tx);
        c.status = ConnStatus::Opening;
        self.status = format!("Connecting to {}…", c.cfg.name);
        if self.ws.sel.cid != Some(cid) {
            self.ws.sel = Sel::conn(cid);
        }
    }

    pub(super) fn close_conn(&mut self, cid: u64) {
        if let Some(c) = self.ws.conn_mut(cid) {
            c.tx = None;
            c.status = ConnStatus::Closed;
            c.dbs.clear();
            c.expanded = false;
            c.users = None;
        }
        self.ws.tabs.retain(|t| t.cid() != cid);
        self.ws.active = self.ws.active.min(self.ws.tabs.len());
        if self.ws.sel.cid == Some(cid) {
            self.ws.sel = Sel::conn(cid);
        }
    }

    pub(super) fn toggle_conn(&mut self, cid: u64) {
        let Some(c) = self.ws.conn_mut(cid) else { return };
        if c.is_open() {
            c.expanded = !c.expanded;
        } else {
            self.open_conn(cid);
        }
    }

    pub(super) fn edit_conn(&mut self, cid: u64) {
        if let Some(c) = self.ws.conn(cid) {
            self.dialogs.conn = Some(ConnDialog::new(Some(cid), c.cfg.clone()));
        }
    }

    pub(super) fn new_conn(&mut self, kind: DbKind) {
        self.dialogs.conn = Some(ConnDialog::new(None, ConnConfig::blank(self.ids.next_conn, kind)));
    }

    pub(super) fn dup_conn(&mut self, cid: u64) {
        let Some(c) = self.ws.conn(cid) else { return };
        let cfg = ConnConfig { id: self.ids.next_conn, name: format!("{} (copy)", c.cfg.name), ..c.cfg.clone() };
        self.ids.next_conn += 1;
        self.ws.conns.push(ConnState::new(cfg));
        self.save_config();
    }

    pub(super) fn delete_conn(&mut self, cid: u64) {
        let name = self.ws.conn(cid).map(|c| c.cfg.name.clone()).unwrap_or_default();
        self.confirm("Delete Connection", format!("Are you sure you want to delete the connection \"{}\"?", name), Confirm::DeleteConn(cid));
    }

    pub(super) fn refresh_conn(&mut self, cid: u64) {
        if self.ws.conn(cid).is_some_and(|c| c.is_open()) {
            self.send(cid, Req::Databases);
        }
    }

    pub(super) fn load_users(&mut self, cid: u64) {
        if let Some(c) = self.ws.conn_mut(cid) {
            c.users = None;
        }
        self.send(cid, Req::Users);
    }

    /// Starts a throw-away worker that only connects. Its id is far above real
    /// connection ids so its replies are recognised.
    pub(super) fn test_conn(&mut self, mut cfg: ConnConfig) {
        self.ids.next_tab += 1;
        let id = u64::MAX - self.ids.next_tab;
        cfg.id = id;
        let tx = worker::spawn(cfg, self.rt.reply.clone());
        // A fresh channel cannot be closed yet.
        let _ = tx.send(Req::Connect);
        if let Some(d) = &mut self.dialogs.conn {
            d.test = None;
            d.test_id = Some(id);
        }
        self.rt.test_tx = Some((id, tx));
    }

    pub(super) fn close_conn_dialog(&mut self) {
        self.dialogs.conn = None;
        self.rt.test_tx = None;
    }

    pub(super) fn save_conn(&mut self, editing: Option<u64>, mut cfg: ConnConfig) {
        if cfg.name.trim().is_empty() {
            cfg.name = format!("{}@{}", cfg.user, cfg.host);
        }
        match editing {
            Some(id) => self.replace_conn(id, cfg),
            None => {
                cfg.id = self.ids.next_conn;
                self.ids.next_conn += 1;
                let id = cfg.id;
                self.ws.conns.push(ConnState::new(cfg));
                self.open_conn(id);
            }
        }
        self.save_config();
        self.close_conn_dialog();
    }

    /// Applies edited settings, reconnecting if the connection was open.
    fn replace_conn(&mut self, id: u64, cfg: ConnConfig) {
        let reopen = self.ws.conn(id).is_some_and(|c| c.is_open());
        if reopen {
            self.close_conn(id);
        }
        if let Some(c) = self.ws.conn_mut(id) {
            c.cfg = cfg;
            c.status = ConnStatus::Closed;
        }
        if reopen {
            self.open_conn(id);
        }
    }

    pub(super) fn save_config(&mut self) {
        let cfgs: Vec<ConnConfig> = self.ws.conns.iter().map(|c| c.cfg.clone()).collect();
        if let Err(e) = self.store.save_connections(&cfgs) {
            self.status = e;
        }
    }

    pub(super) fn set_theme(&mut self, pref: ThemePref) {
        self.settings.theme = pref;
        pref.apply(self.ctx());
        if let Err(e) = self.store.save_settings(&self.settings) {
            self.status = e;
        }
    }
}
