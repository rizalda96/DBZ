//! Replies to `Req::Exec`, routed by why the statements were sent.

use super::{Dbz, Tab};
use crate::db::DbError;
use crate::worker::{Purpose, Req};

impl Dbz {
    pub(super) fn exec_done(&mut self, cid: u64, (tab, db): (u64, String), result: Result<u64, DbError>, purpose: Purpose) {
        match (purpose, result) {
            (Purpose::SaveData, Ok(n)) => self.data_saved(cid, tab, n),
            (Purpose::SaveData, Err(e)) => {
                if let Some(Tab::Data(d)) = self.ws.tab_mut(tab) {
                    d.saving = false;
                }
                self.alert("Couldn't save changes", e.to_string());
            }
            (Purpose::SaveMatView, result) => self.matview_saved(cid, (tab, db), result),
            (Purpose::SaveView, result) => self.view_saved(cid, (tab, db), result),
            (Purpose::SaveDesign { new_name }, Ok(_)) => self.design_saved(cid, (tab, db), new_name),
            (Purpose::SaveDesign { .. }, Err(e)) => {
                if let Some(Tab::Design(d)) = self.ws.tab_mut(tab) {
                    d.saving = false;
                    d.error = Some(e.to_string());
                }
                self.alert("Couldn't save the table", e.to_string());
            }
            (_, Ok(n)) => {
                if !db.is_empty() {
                    self.refresh_db(cid, db);
                }
                self.refresh_conn(cid);
                self.status = format!("Done ({} row(s) affected)", n);
            }
            (_, Err(e)) => self.alert("Error", e.to_string()),
        }
    }

    /// Grid changes were applied: reload the page.
    fn data_saved(&mut self, cid: u64, tab: u64, n: u64) {
        let req = match self.ws.tab_mut(tab) {
            Some(Tab::Data(d)) => {
                d.saving = false;
                Some(d.reload_req())
            }
            _ => None,
        };
        if let Some(r) = req {
            self.send(cid, r);
        }
        self.status = format!("Changes saved ({} row(s) affected)", n);
    }

    /// A table design was saved: reload its columns, the object list and its data tabs.
    fn design_saved(&mut self, cid: u64, (tab, db): (u64, String), new_name: String) {
        if let Some(Tab::Design(d)) = self.ws.tab_mut(tab) {
            d.saving = false;
            d.table = Some(new_name.clone());
            d.name = new_name.clone();
            d.loading = true;
            d.parts.loading = true;
            d.orig_comment = d.comment.clone();
        }
        self.send(cid, Req::Columns { db: db.clone(), table: new_name.clone() });
        self.send(cid, Req::TableParts { db: db.clone(), table: new_name.clone() });
        self.status = format!("Table \"{}\" saved", new_name);
        self.refresh_db(cid, db.clone());
        let reloads: Vec<Req> = self
            .ws
            .tabs
            .iter_mut()
            .filter_map(|t| match t {
                Tab::Data(d) if d.cid == cid && d.db == db && d.table == new_name => Some(d.reload_req()),
                _ => None,
            })
            .collect();
        for r in reloads {
            self.send(cid, r);
        }
    }
}
