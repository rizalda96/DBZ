//! Functions and procedures: open a definition in a query tab, or a statement that
//! runs one.

use super::{Dbz, ObjRef, Tab};
use crate::db::DbError;
use crate::domain::RoutineInfo;
use crate::worker::Req;

impl Dbz {
    /// Asks for the definition; `routine_source` opens it when it arrives.
    pub(super) fn design_routine(&mut self, obj: ObjRef) {
        let Some(routine) = self.routine(&obj) else { return };
        self.send(obj.cid, Req::RoutineSource { db: obj.db, routine });
    }

    pub(super) fn routine_source(&mut self, cid: u64, db: String, name: String, result: Result<String, DbError>) {
        match result {
            Ok(sql) => self.named_query(cid, db, sql, name),
            Err(e) => self.alert("Design Function", e.to_string()),
        }
    }

    /// A query tab with a call to the routine, for the user to fill in the arguments.
    pub(super) fn exec_routine(&mut self, obj: ObjRef) {
        let Some(routine) = self.routine(&obj) else { return };
        let sql = self.ws.dialect_of(obj.cid).routine_call(&routine);
        self.named_query(obj.cid, obj.db, sql, routine.name);
    }

    /// The function or procedure `obj` names, as the tree listed it.
    fn routine(&mut self, obj: &ObjRef) -> Option<RoutineInfo> {
        let found = self.ws.conn(obj.cid).and_then(|c| c.db(&obj.db)).and_then(|d| d.routines.iter().find(|r| r.name == obj.name)).cloned();
        if found.is_none() {
            self.alert("Function", format!("\"{}\" is not in the list any more. Refresh the database (F5) and try again.", obj.name));
        }
        found
    }

    /// A new query tab titled after the routine it shows.
    fn named_query(&mut self, cid: u64, db: String, sql: String, title: String) {
        self.new_query(cid, db, sql);
        if let Some(Tab::Query(q)) = self.ws.tabs.last_mut() {
            q.title = title;
        }
    }
}
