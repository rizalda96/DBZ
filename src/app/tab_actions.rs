//! Work tabs: data, designer and query tabs; saving queries; closing tabs.

use super::{Confirm, Dbz, Dialog, ObjRef, Tab};
use crate::domain::RelationKind;
use crate::shared::sanitize;
use crate::ui::views::{DataTab, DesignTab, MatViewTab, QueryTab};
use crate::worker::Req;

impl Dbz {
    /// Brings tab `i` (0-based) to the front.
    fn focus_tab(&mut self, i: usize) {
        self.ws.active = i + 1;
    }

    fn push_tab(&mut self, tab: Tab) {
        self.ws.tabs.push(tab);
        self.ws.active = self.ws.tabs.len();
    }

    pub(super) fn open_table(&mut self, obj: ObjRef, is_view: bool) {
        let open = self.ws.tabs.iter().position(|t| matches!(t, Tab::Data(d) if d.cid == obj.cid && d.db == obj.db && d.table == obj.name));
        if let Some(i) = open {
            return self.focus_tab(i);
        }
        let kind = self.ws.kind_of(obj.cid);
        let mut t = DataTab::new(self.new_tab_id(), obj.cid, kind, obj.db, obj.name);
        t.is_view = is_view;
        let req = t.reload_req();
        self.push_tab(Tab::Data(t));
        self.send(obj.cid, req);
    }

    pub(super) fn design_table(&mut self, cid: u64, db: String, table: Option<String>) {
        let open = table.as_ref().and_then(|tn| self.ws.tabs.iter().position(|t| matches!(t, Tab::Design(d) if d.cid == cid && d.db == db && d.table.as_ref() == Some(tn))));
        if let Some(i) = open {
            return self.focus_tab(i);
        }
        let kind = self.ws.kind_of(cid);
        let id = self.new_tab_id();
        self.send(cid, Req::IndexChoices(db.clone()));
        let Some(tn) = table else {
            return self.push_tab(Tab::Design(DesignTab::new_table(id, cid, kind, db)));
        };
        let comment = self.ws.conn(cid).and_then(|c| c.db(&db)).and_then(|d| d.tables.iter().find(|t| t.name == tn)).map(|t| t.comment.clone()).unwrap_or_default();
        self.push_tab(Tab::Design(DesignTab::existing(id, cid, kind, db.clone(), (tn.clone(), comment))));
        self.send(cid, Req::Columns { db: db.clone(), table: tn.clone() });
        self.send(cid, Req::TableParts { db, table: tn });
    }

    pub(super) fn ask_drop_tables(&mut self, cid: u64, db: String, names: Vec<String>, kind: RelationKind) {
        let msg = match names.as_slice() {
            [one] => format!("Are you sure you want to delete the {} \"{}\"?", kind.noun(), one),
            _ => format!("Are you sure you want to delete these {} {}s?", names.len(), kind.noun()),
        };
        self.confirm(format!("Delete {}", kind.title()), msg, Confirm::DropTables(cid, db, names, kind));
    }

    pub(super) fn ask_truncate(&mut self, obj: ObjRef) {
        let msg = format!("Are you sure you want to remove every record from \"{}\"? This can't be undone.", obj.name);
        self.confirm("Truncate Table", msg, Confirm::Truncate(obj));
    }

    pub(super) fn ask_drop_routine(&mut self, obj: ObjRef, kind: String) {
        let k = kind.to_lowercase();
        let msg = format!("Are you sure you want to delete the {} \"{}\"?", k, obj.name);
        self.confirm(format!("Delete {}", k), msg, Confirm::DropRoutine(obj, kind));
    }

    /// New query tab; with no database given, the first expanded (or first) one.
    pub(super) fn new_query(&mut self, cid: u64, db: String, sql: String) {
        self.ids.query_counter += 1;
        let q = self.editor_state(cid, db, sql);
        self.push_editor_tab(Tab::Query(q));
    }

    /// New materialized view designer in `db`, in the schema selected there.
    pub(super) fn new_matview(&mut self, cid: u64, db: String) {
        let q = self.editor_state(cid, db, String::new());
        let db = q.db.clone();
        self.push_editor_tab(Tab::MatView(MatViewTab::new(q)));
        self.send(cid, Req::IndexChoices(db));
    }

    /// An SQL editor's state in `db` (the first expanded or first database when empty),
    /// in the schema the tree has selected.
    pub(super) fn editor_state(&mut self, cid: u64, db: String, sql: String) -> QueryTab {
        let db = if db.is_empty() { self.ws.conn(cid).and_then(|c| c.dbs.iter().find(|d| d.expanded).or(c.dbs.first())).map(|d| d.name.clone()).unwrap_or_default() } else { db };
        let mut q = QueryTab::new(self.new_tab_id(), cid, db, sql);
        q.schema = self.query_schema(cid, &q.db);
        q
    }

    /// Opens a tab with an SQL editor; when its database's schemas aren't known yet they
    /// are asked for, and its schema is filled in when they arrive (`replies::schemas`).
    pub(super) fn push_editor_tab(&mut self, mut tab: Tab) {
        let cid = tab.cid();
        let missing = tab.query_mut().filter(|q| q.schema.is_empty() && !q.db.is_empty()).map(|q| q.db.clone());
        self.push_tab(tab);
        self.ensure_open(cid);
        if let Some(db) = missing {
            self.send(cid, Req::Schemas(db));
        }
    }

    /// The schema the tree has selected in `db`, else the database's default one.
    pub(super) fn query_schema(&self, cid: u64, db: &str) -> String {
        let sel = &self.ws.sel;
        let selected = sel.schema.clone().filter(|_| sel.cid == Some(cid) && sel.db.as_deref() == Some(db));
        selected.or_else(|| self.ws.conn(cid).and_then(|c| c.db(db)).map(|d| d.default_schema())).unwrap_or_default()
    }

    pub(super) fn open_saved_query(&mut self, obj: ObjRef) {
        let open = self.ws.tabs.iter().position(|t| matches!(t, Tab::Query(q) if q.cid == obj.cid && q.db == obj.db && q.saved_name.as_deref() == Some(obj.name.as_str())));
        if let Some(i) = open {
            return self.focus_tab(i);
        }
        let path = self.store.folder("queries", obj.cid, &obj.db).join(format!("{}.sql", obj.name));
        let sql = match std::fs::read_to_string(&path) {
            Ok(sql) => sql,
            Err(e) => return self.alert("Open Query", format!("Could not read {}: {}", path.display(), e)),
        };
        self.new_query(obj.cid, obj.db, sql);
        if let Some(Tab::Query(q)) = self.ws.tabs.last_mut() {
            q.title = obj.name.clone();
            q.saved_name = Some(obj.name);
        }
    }

    pub(super) fn ask_delete_query(&mut self, obj: ObjRef) {
        let msg = format!("Delete the saved query \"{}\"?", obj.name);
        self.confirm("Delete Query", msg, Confirm::DeleteQuery(obj));
    }

    /// Saves under the existing name, or asks for one.
    pub(super) fn save_query(&mut self, tab: u64) {
        let Some(Tab::Query(q)) = self.ws.tab_mut(tab) else { return };
        if let Some(name) = q.saved_name.clone() {
            return self.write_query(tab, name);
        }
        let name = if q.title == "Untitled" { format!("Query {}", self.ids.query_counter) } else { q.title.clone() };
        self.dialogs.dialog = Some(Dialog::SaveQuery { tab, name });
    }

    pub(super) fn write_query(&mut self, tab: u64, name: String) {
        let Some(Tab::Query(q)) = self.ws.tab_mut(tab) else { return };
        let (cid, db, sql) = (q.cid, q.db.clone(), q.sql.clone());
        let dir = self.store.folder("queries", cid, &db);
        let clean = sanitize(&name);
        if let Err(e) = std::fs::create_dir_all(&dir).and_then(|_| std::fs::write(dir.join(format!("{}.sql", clean)), sql)) {
            return self.alert("Save Query", e.to_string());
        }
        if let Some(Tab::Query(q)) = self.ws.tab_mut(tab) {
            q.saved_name = Some(clean.clone());
            q.title = clean;
            q.dirty = false;
        }
        self.status = format!("Saved query \"{}\"", name);
        self.scan_files(cid, &db);
    }

    /// Closes tab `i` (1-based), asking first when it has unsaved work.
    pub(super) fn ask_close_tab(&mut self, i: usize) {
        let Some(tab) = i.checked_sub(1).and_then(|k| self.ws.tabs.get(k)) else { return };
        if tab.has_unsaved() {
            self.confirm("Close Tab", "This tab has unsaved changes. Close it anyway?", Confirm::CloseDirtyTab(i));
        } else {
            self.close_tab(i);
        }
    }

    pub(super) fn close_tab(&mut self, i: usize) {
        if i == 0 || i > self.ws.tabs.len() {
            return;
        }
        self.ws.tabs.remove(i - 1);
        if self.ws.active >= i {
            self.ws.active = self.ws.active.saturating_sub(1);
        }
        self.ws.active = self.ws.active.min(self.ws.tabs.len());
    }
}
