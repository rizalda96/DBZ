//! Runs a destructive action after the user pressed OK.

use super::{Act, Confirm, Dbz, Sel, Tab};
use crate::worker::{Purpose, Req};

impl Dbz {
    pub(super) fn run_confirm(&mut self, action: Confirm) {
        match action {
            Confirm::DropTables(cid, db, names, view) => self.drop_tables(cid, db, names, view),
            Confirm::DropDb(cid, db) => {
                let sql = format!("DROP DATABASE {}", self.ws.dialect_of(cid).quote_ident(&db));
                self.exec(cid, String::new(), sql, Purpose::Other("database".into()));
                if self.ws.sel.db.as_deref() == Some(db.as_str()) {
                    self.ws.sel = Sel::conn(cid);
                }
            }
            Confirm::DeleteConn(cid) => {
                self.do_act(Act::CloseConn(cid));
                self.ws.conns.retain(|c| c.cfg.id != cid);
                if self.ws.sel.cid == Some(cid) {
                    self.ws.sel = Sel::default();
                }
                self.save_config();
            }
            Confirm::Truncate(obj) => {
                let sql = format!("TRUNCATE TABLE {}", self.ws.dialect_of(obj.cid).quote_ident(&obj.name));
                self.exec(obj.cid, obj.db, sql, Purpose::Refresh);
            }
            Confirm::DeleteQuery(obj) => self.delete_saved("queries", obj),
            Confirm::DeleteBackup(obj) => self.delete_saved("backups", obj),
            Confirm::RestoreBackup(obj) => self.restore_backup(obj),
            Confirm::RestoreArchive(cid, db, path) => self.send(cid, Req::RestoreArchive { db, path }),
            Confirm::DropRoutine(obj, kind) => {
                let kw = if kind.eq_ignore_ascii_case("procedure") { "PROCEDURE" } else { "FUNCTION" };
                let sql = format!("DROP {} {}", kw, self.ws.dialect_of(obj.cid).quote_ident(&obj.name));
                self.exec(obj.cid, obj.db, sql, Purpose::Refresh);
            }
            Confirm::CloseDirtyTab(i) => self.close_tab(i),
        }
    }

    /// Runs one statement that is not tied to a tab.
    fn exec(&mut self, cid: u64, db: String, sql: String, purpose: Purpose) {
        self.send(cid, Req::Exec { tab: 0, db, stmts: vec![sql], purpose });
    }

    fn drop_tables(&mut self, cid: u64, db: String, names: Vec<String>, view: bool) {
        let d = self.ws.dialect_of(cid);
        let kw = if view { "VIEW" } else { "TABLE" };
        let stmts = names.iter().map(|n| format!("DROP {} {}", kw, d.quote_ident(n))).collect();
        self.send(cid, Req::Exec { tab: 0, db: db.clone(), stmts, purpose: Purpose::DropTables });
        self.ws.tabs.retain(|t| !matches!(t, Tab::Data(d) if d.cid == cid && d.db == db && names.contains(&d.table)));
        self.ws.active = self.ws.active.min(self.ws.tabs.len());
        self.ws.sel.obj = None;
    }
}
