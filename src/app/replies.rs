//! Applies worker replies to the app state.

use std::time::Duration;

use super::{ConnStatus, Dbz, DbState, Tab};
use crate::db::DbError;
use crate::domain::{FieldDef, IndexChoices, ResultSet, RoutineInfo, TableInfo, TableParts};
use crate::ui::views::RowState;
use crate::worker::{Resp, TablePage};

/// Statements after which the object lists may be out of date.
const SCHEMA_CHANGING: [&str; 8] = ["CREATE", "DROP", "ALTER", "RENAME", "INSERT", "DELETE", "TRUNCATE", "UPDATE"];

impl Dbz {
    /// Handles every reply that has arrived since the last frame.
    pub(super) fn poll(&mut self) {
        while let Ok((cid, resp)) = self.rt.rx.try_recv() {
            if self.dialogs.conn.as_ref().is_some_and(|d| d.test_id == Some(cid)) {
                self.test_reply(resp);
            } else if cid <= u64::MAX / 2 {
                // Higher ids belong to finished connection tests and are dropped.
                self.handle(cid, resp);
            }
        }
    }

    fn test_reply(&mut self, resp: Resp) {
        let Some(d) = &mut self.dialogs.conn else { return };
        match resp {
            Resp::Connected { version } if version.is_empty() => d.test = Some(Ok("Connection successful".into())),
            Resp::Connected { version } => d.test = Some(Ok(format!("Connection successful — server {}", version))),
            Resp::ConnectFailed(e) => d.test = Some(Err(e.to_string())),
            _ => {}
        }
        self.rt.test_tx = None;
    }

    fn handle(&mut self, cid: u64, resp: Resp) {
        match resp {
            Resp::Connected { version } => self.connected(cid, version),
            Resp::ConnectFailed(e) => self.connect_failed(cid, e),
            Resp::Databases(Ok(list)) => self.databases(cid, list),
            Resp::Databases(Err(e)) => self.alert("Databases", e.to_string()),
            Resp::Tables(db, r) => self.tables(cid, &db, r),
            Resp::Routines(db, r) => self.routines(cid, &db, r),
            Resp::Schemas(db, r) => self.schemas(cid, &db, r),
            Resp::Columns { db, table, result } => self.columns(cid, (&db, &table), result),
            Resp::TableParts { db, table, result } => self.table_parts(cid, (&db, &table), result),
            Resp::IndexChoices(db, r) => self.index_choices(cid, &db, r),
            Resp::RefColumns { db, schema, table, result } => self.ref_columns(cid, &db, (&schema, &table), result),
            Resp::Ddl { db, table, result } => {
                self.ws.ddl_cache.insert((cid, db, table), result);
            }
            Resp::Query { tab, results, error, elapsed } => self.query_done(cid, tab, (results, error), elapsed),
            Resp::TableData { tab, result, sql, elapsed } => self.table_data(tab, result, sql, elapsed),
            Resp::Exec { tab, db, result, purpose } => self.exec_done(cid, (tab, db), result, purpose),
            Resp::Task(r) => self.task_done(cid, r),
            Resp::Users(r) => {
                if let Some(c) = self.ws.conn_mut(cid) {
                    c.users = Some(r);
                }
            }
        }
    }

    fn connected(&mut self, cid: u64, version: String) {
        let Some(c) = self.ws.conn_mut(cid) else { return };
        c.status = ConnStatus::Open;
        c.version = version;
        self.status = format!("Connected to {}", c.cfg.name);
    }

    /// Closes the "please wait" window of a task on `cid`.
    fn end_task(&mut self, cid: u64) {
        if self.dialogs.task.as_ref().is_some_and(|t| t.cid == cid) {
            self.dialogs.task = None;
        }
    }

    fn connect_failed(&mut self, cid: u64, e: DbError) {
        self.end_task(cid);
        let msg = e.to_string();
        let name = self.ws.conn(cid).map(|c| c.cfg.name.clone()).unwrap_or_default();
        if let Some(c) = self.ws.conn_mut(cid) {
            c.status = ConnStatus::Failed(msg.clone());
            c.tx = None;
            c.expanded = false;
        }
        for t in self.ws.tabs.iter_mut().filter(|t| t.cid() == cid) {
            match t {
                Tab::Query(q) => q.running = false,
                Tab::Data(d) => {
                    d.loading = false;
                    d.error = Some(msg.clone());
                }
                Tab::Design(d) => {
                    d.loading = false;
                    d.parts.loading = false;
                    d.saving = false;
                }
            }
        }
        self.alert(format!("Can't connect to \"{}\"", name), msg);
        self.status = "Connection failed".into();
    }

    /// Replaces the database list, keeping the state of databases that still exist.
    fn databases(&mut self, cid: u64, list: Vec<String>) {
        if let Some(c) = self.ws.conn_mut(cid) {
            let mut old: Vec<DbState> = std::mem::take(&mut c.dbs);
            c.dbs = list.into_iter().map(|n| old.iter().position(|d| d.name == n).map(|i| old.remove(i)).unwrap_or_else(|| DbState::new(n))).collect();
        }
    }

    fn tables(&mut self, cid: u64, db: &str, r: Result<Vec<TableInfo>, DbError>) {
        let Some(d) = self.ws.db_mut(cid, db) else { return };
        d.loading = false;
        match r {
            Ok(t) => {
                d.tables = t;
                d.loaded = true;
                d.error = None;
            }
            Err(e) => {
                d.error = Some(e.to_string());
                self.status = format!("Error loading tables: {}", e);
            }
        }
    }

    fn routines(&mut self, cid: u64, db: &str, r: Result<Vec<RoutineInfo>, DbError>) {
        if let Some(d) = self.ws.db_mut(cid, db) {
            d.routines = r.unwrap_or_default();
        }
    }

    /// Stores the schemas, and starts query tabs of `db` that have none yet in its
    /// default one.
    fn schemas(&mut self, cid: u64, db: &str, r: Result<Vec<String>, DbError>) {
        let Some(d) = self.ws.db_mut(cid, db) else { return };
        match r {
            Ok(s) => d.schemas = s,
            Err(e) => {
                self.status = format!("Error loading schemas: {}", e);
                return;
            }
        }
        let default = d.default_schema();
        let waiting = self.ws.tabs.iter_mut().filter_map(|t| match t {
            Tab::Query(q) if q.cid == cid && q.db == db && q.schema.is_empty() => Some(q),
            _ => None,
        });
        for q in waiting {
            q.schema.clone_from(&default);
        }
    }

    /// Fills designer tabs that were waiting for these columns.
    fn columns(&mut self, cid: u64, (db, table): (&str, &str), result: Result<Vec<FieldDef>, DbError>) {
        let waiting = self.ws.tabs.iter_mut().filter_map(|t| match t {
            Tab::Design(d) if d.cid == cid && d.db == db && d.table.as_deref() == Some(table) && d.loading => Some(d),
            _ => None,
        });
        for d in waiting {
            d.loading = false;
            match &result {
                Ok(f) => {
                    d.fields = f.clone();
                    d.orig = f.clone();
                }
                Err(e) => d.error = Some(e.to_string()),
            }
        }
    }

    /// Fills designer tabs that were waiting for the table's indexes, keys and triggers.
    fn table_parts(&mut self, cid: u64, (db, table): (&str, &str), result: Result<TableParts, DbError>) {
        let waiting = self.ws.tabs.iter_mut().filter_map(|t| match t {
            Tab::Design(d) if d.cid == cid && d.db == db && d.table.as_deref() == Some(table) && d.parts.loading => Some(d),
            _ => None,
        });
        for d in waiting {
            match &result {
                Ok(p) => d.parts.loaded(p.clone()),
                Err(e) => {
                    d.parts.loading = false;
                    d.error = Some(format!("Could not read indexes, keys and triggers: {}", e));
                }
            }
        }
    }

    /// Gives designer tabs of `db` the drop-down lists of the index fields editor.
    fn index_choices(&mut self, cid: u64, db: &str, r: Result<IndexChoices, DbError>) {
        let choices = match r {
            Ok(c) => c,
            Err(e) => {
                self.status = format!("Could not read collations and operator classes: {}", e);
                return;
            }
        };
        for t in self.ws.tabs.iter_mut() {
            if let Tab::Design(d) = t {
                if d.cid == cid && d.db == db {
                    d.index_choices = choices.clone();
                }
            }
        }
    }

    /// Gives designer tabs of `db` the columns of a table their foreign keys reference.
    fn ref_columns(&mut self, cid: u64, db: &str, (schema, table): (&str, &str), r: Result<Vec<String>, DbError>) {
        let columns = r.unwrap_or_else(|e| {
            self.status = format!("Could not read the columns of {}: {}", table, e);
            vec![]
        });
        let tabs = self.ws.tabs.iter_mut().filter_map(|t| match t {
            Tab::Design(d) if d.cid == cid && d.db == db => Some(d),
            _ => None,
        });
        for d in tabs {
            d.ref_columns.arrived(schema, table, columns.clone());
        }
    }

    fn query_done(&mut self, cid: u64, tab: u64, (results, error): (Vec<ResultSet>, Option<(String, DbError)>), elapsed: Duration) {
        let Some(Tab::Query(q)) = self.ws.tab_mut(tab) else { return };
        q.running = false;
        q.elapsed = Some(elapsed);
        q.view = if error.is_some() { 0 } else { results.iter().position(|r| r.has_rows).map(|i| i + 1).unwrap_or(0) };
        q.sel_cell = None;
        let changed_schema = results.iter().any(|r| {
            let u = r.statement.trim_start().to_uppercase();
            SCHEMA_CHANGING.iter().any(|k| u.starts_with(k))
        });
        self.status = match &error {
            Some((_, e)) => format!("Query failed: {}", e.message().lines().next().unwrap_or("")),
            None => format!("Query finished in {:.3}s", elapsed.as_secs_f64()),
        };
        q.results = results;
        q.error = error.map(|(st, e)| (st, e.to_string()));
        let db = q.db.clone();
        if changed_schema {
            self.refresh_conn(cid);
            if self.ws.conn(cid).and_then(|c| c.db(&db)).is_some_and(|d| d.loaded) {
                self.refresh_db(cid, db);
            }
        }
    }

    fn table_data(&mut self, tab: u64, result: Result<TablePage, DbError>, sql: String, elapsed: Duration) {
        let Some(Tab::Data(d)) = self.ws.tab_mut(tab) else { return };
        d.loading = false;
        d.elapsed = Some(elapsed);
        d.last_sql = sql;
        let (fields, rs, total) = match result {
            Ok(page) => page,
            Err(e) => {
                d.error = Some(e.to_string());
                return;
            }
        };
        d.fields = fields;
        d.columns = rs.columns;
        d.rows = rs.rows.into_iter().map(RowState::loaded).collect();
        d.total = total;
        d.error = None;
        d.editing = None;
        if d.sel.is_some_and(|(r, _)| r >= d.rows.len()) {
            d.sel = None;
        }
    }

    fn task_done(&mut self, cid: u64, r: Result<String, DbError>) {
        self.end_task(cid);
        match r {
            Ok(msg) => {
                self.status = msg.clone();
                self.alert("Finished", msg);
            }
            Err(e) => self.alert("Error", e.to_string()),
        }
        let dbs: Vec<String> = self.ws.conn(cid).map(|c| c.dbs.iter().filter(|d| d.loaded).map(|d| d.name.clone()).collect()).unwrap_or_default();
        for db in dbs {
            self.refresh_db(cid, db);
        }
    }
}
