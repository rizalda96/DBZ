//! Saving a materialized view designer: name it on the first save, run its CREATE.

use super::{Dbz, Dialog, Tab};
use crate::db::DbError;
use crate::worker::{Purpose, Req};

impl Dbz {
    /// Saves under the view's name, or asks for one first.
    pub(super) fn save_matview(&mut self, tab: u64) {
        let Some(Tab::MatView(m)) = self.ws.tab_mut(tab) else { return };
        if m.def.name.trim().is_empty() {
            self.dialogs.dialog = Some(Dialog::NameMatView { tab, name: String::new() });
            return;
        }
        self.create_matview(tab);
    }

    /// The name typed in the Save dialog.
    pub(super) fn name_matview(&mut self, tab: u64, name: String) {
        if let Some(Tab::MatView(m)) = self.ws.tab_mut(tab) {
            m.def.name = name;
        }
        self.create_matview(tab);
    }

    fn create_matview(&mut self, tab: u64) {
        let Some(Tab::MatView(m)) = self.ws.tab_mut(tab) else { return };
        let stmts = m.statements();
        if stmts.is_empty() {
            return;
        }
        m.saving = true;
        m.error = None;
        let (cid, db) = (m.cid, m.query.db.clone());
        self.send(cid, Req::Exec { tab, db, stmts, purpose: Purpose::SaveMatView });
    }

    /// The server's answer to the CREATE.
    pub(super) fn matview_saved(&mut self, cid: u64, (tab, db): (u64, String), result: Result<u64, DbError>) {
        let Some(Tab::MatView(m)) = self.ws.tab_mut(tab) else { return };
        match result {
            Ok(_) => {
                m.saved_ok();
                self.status = format!("Materialized view \"{}\" saved", m.shown_name());
                self.refresh_db(cid, db);
            }
            Err(e) => {
                m.saving = false;
                m.error = Some(e.to_string());
                self.alert("Couldn't save the materialized view", e.to_string());
            }
        }
    }
}
