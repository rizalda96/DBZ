//! Databases, folders, the object mode and the DDL cache.

use super::{Confirm, Dbz, Dialog, ObjMode, ObjRef, Sel};
use crate::worker::Req;

impl Dbz {
    pub(super) fn toggle_db(&mut self, cid: u64, db: String) {
        match self.ws.db_mut(cid, &db) {
            Some(d) if d.expanded => d.expanded = false,
            _ => self.open_db(cid, db),
        }
    }

    pub(super) fn open_db(&mut self, cid: u64, db: String) {
        let load = self.ws.db_mut(cid, &db).is_some_and(|d| {
            d.expanded = true;
            let load = !d.loaded && !d.loading;
            d.loading |= load;
            load
        });
        if load {
            self.load_db(cid, &db);
        }
        if let Some(c) = self.ws.conn_mut(cid) {
            c.expanded = true;
        }
        self.ws.sel = Sel::db(cid, db);
        if self.ws.mode == ObjMode::Users {
            self.ws.mode = ObjMode::Tables;
        }
    }

    /// Asks for tables and routines and re-reads saved files.
    fn load_db(&mut self, cid: u64, db: &str) {
        self.send(cid, Req::Tables(db.to_string()));
        self.send(cid, Req::Routines(db.to_string()));
        self.scan_files(cid, db);
    }

    pub(super) fn close_db(&mut self, cid: u64, db: &str) {
        if let Some(d) = self.ws.db_mut(cid, db) {
            d.expanded = false;
            d.loaded = false;
            d.tables.clear();
            d.routines.clear();
        }
    }

    pub(super) fn refresh_db(&mut self, cid: u64, db: String) {
        if let Some(d) = self.ws.db_mut(cid, &db) {
            d.loading = true;
        }
        self.load_db(cid, &db);
        self.ws.ddl_cache.retain(|k, _| !(k.0 == cid && k.1 == db));
    }

    pub(super) fn new_db(&mut self, cid: u64) {
        let charset = self.ws.dialect_of(cid).default_charset().to_string();
        self.dialogs.dialog = Some(Dialog::NewDb { cid, name: String::new(), charset });
    }

    pub(super) fn drop_db(&mut self, cid: u64, db: String) {
        let msg = format!("Are you sure you want to delete the database \"{}\"? All of its tables and data will be lost.", db);
        self.confirm("Delete Database", msg, Confirm::DropDb(cid, db));
    }

    pub(super) fn select_folder(&mut self, cid: u64, db: String, mode: ObjMode) {
        self.ws.mode = mode;
        self.ws.active = 0;
        self.ws.sel = Sel::db(cid, db);
    }

    pub(super) fn toggle_folder(&mut self, cid: u64, db: &str, mode: ObjMode) {
        let Some(d) = self.ws.db_mut(cid, db) else { return };
        if !d.open_folders.remove(&mode) {
            d.open_folders.insert(mode);
        }
    }

    pub(super) fn set_mode(&mut self, m: ObjMode) {
        if self.ws.mode != m {
            self.ws.sel.obj = None;
        }
        self.ws.mode = m;
        self.ws.active = 0;
        if let (ObjMode::Users, Some(cid)) = (m, self.ws.sel.cid) {
            self.load_users(cid);
        }
        if let (ObjMode::Queries | ObjMode::Backups, Some((cid, db))) = (m, self.ws.sel.conn_db()) {
            self.scan_files(cid, &db);
        }
    }

    /// Fetches DDL for the info pane, with an empty placeholder while it loads.
    pub(super) fn load_ddl(&mut self, obj: ObjRef, is_view: bool) {
        self.ws.ddl_cache.insert((obj.cid, obj.db.clone(), obj.name.clone()), Ok(String::new()));
        self.send(obj.cid, Req::Ddl { db: obj.db, table: obj.name, is_view });
    }
}
