//! Design View: open a view's query in its own tab, and save it back with Run.

use super::{Dbz, ObjRef, Tab};
use crate::db::DbError;
use crate::sql::split_view;
use crate::ui::views::ViewTab;
use crate::worker::{Purpose, Req};

impl Dbz {
    /// Asks for a view's definition; `view_source` opens it when it arrives.
    pub(super) fn design_view(&mut self, obj: ObjRef) {
        self.send(obj.cid, Req::ViewSource { db: obj.db, view: obj.name });
    }

    /// A Design View tab with the view's query. A definition DBZ can't split (no
    /// `VIEW … AS`) opens whole in a query tab instead, so it can still be read.
    pub(super) fn view_source(&mut self, cid: u64, db: String, view: String, result: Result<String, DbError>) {
        let ddl = match result {
            Ok(ddl) => ddl,
            Err(e) => return self.alert("Design View", e.to_string()),
        };
        let Some((head, query)) = split_view(self.ws.dialect_of(cid), &ddl) else {
            return self.new_query(cid, db, ddl);
        };
        let q = self.editor_state(cid, db, query);
        self.push_editor_tab(Tab::View(ViewTab::new(q, view, head)));
    }

    pub(super) fn run_view(&mut self, tab: u64) {
        let Some(Tab::View(v)) = self.ws.tab_mut(tab) else { return };
        v.saving = true;
        v.error = None;
        let (cid, db, stmts) = (v.cid, v.query.db.clone(), vec![v.statement()]);
        self.send(cid, Req::Exec { tab, db, stmts, purpose: Purpose::SaveView });
    }

    pub(super) fn view_saved(&mut self, cid: u64, (tab, db): (u64, String), result: Result<u64, DbError>) {
        let Some(Tab::View(v)) = self.ws.tab_mut(tab) else { return };
        match result {
            Ok(_) => {
                v.saved_ok();
                self.status = format!("View \"{}\" saved", v.name);
                self.refresh_db(cid, db);
            }
            Err(e) => {
                v.saving = false;
                v.error = Some(e.to_string());
                self.alert("Couldn't save the view", e.to_string());
            }
        }
    }
}
