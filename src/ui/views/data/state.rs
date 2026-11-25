//! State of a table data tab: loaded rows, edits and paging. No egui here, so the
//! edit-to-SQL logic can be tested directly.

use std::time::Duration;

use crate::domain::{Cell, DbKind, FieldDef, Row};
use crate::sql::{dialect_for, eq_condition, insert_row, Dialect};
use crate::worker::{PageQuery, Req};

const PAGE_SIZE: usize = 1000;

pub struct RowState {
    /// The row as loaded; None for a row added in the grid.
    pub orig: Option<Row>,
    pub cur: Row,
    pub deleted: bool,
}

impl RowState {
    pub fn loaded(row: Row) -> Self {
        RowState { orig: Some(row.clone()), cur: row, deleted: false }
    }

    pub fn changed(&self) -> bool {
        self.deleted || self.orig.as_ref() != Some(&self.cur)
    }
}

/// A cell being edited in place.
pub struct CellEdit {
    pub row: usize,
    pub col: usize,
    pub text: String,
    /// Focus the editor on the next frame.
    pub focus: bool,
}

pub struct DataTab {
    pub id: u64,
    pub cid: u64,
    pub db: String,
    pub table: String,
    pub is_view: bool,
    pub kind: DbKind,
    pub fields: Vec<FieldDef>,
    pub columns: Vec<String>,
    pub rows: Vec<RowState>,
    pub sel: Option<(usize, usize)>,
    pub editing: Option<CellEdit>,
    pub filter: String,
    pub filter_applied: String,
    pub show_filter: bool,
    pub order: Option<(String, bool)>,
    pub limit: usize,
    pub offset: usize,
    pub total: Option<u64>,
    pub loading: bool,
    pub saving: bool,
    pub last_sql: String,
    pub error: Option<String>,
    pub elapsed: Option<Duration>,
    pub scroll_to: Option<usize>,
}

impl DataTab {
    pub fn new(id: u64, cid: u64, kind: DbKind, db: String, table: String) -> Self {
        DataTab {
            id,
            cid,
            db,
            table,
            is_view: false,
            kind,
            fields: vec![],
            columns: vec![],
            rows: vec![],
            sel: None,
            editing: None,
            filter: String::new(),
            filter_applied: String::new(),
            show_filter: false,
            order: None,
            limit: PAGE_SIZE,
            offset: 0,
            total: None,
            loading: false,
            saving: false,
            last_sql: String::new(),
            error: None,
            elapsed: None,
            scroll_to: None,
        }
    }

    fn dialect(&self) -> &'static dyn Dialect {
        dialect_for(self.kind)
    }

    /// Marks the tab as loading and returns the request for the current page.
    pub fn reload_req(&mut self) -> Req {
        self.loading = true;
        let page = PageQuery { table: self.table.clone(), filter: self.filter_applied.clone(), order: self.order.clone(), limit: self.limit, offset: self.offset };
        Req::TableData { tab: self.id, db: self.db.clone(), page }
    }

    pub fn dirty(&self) -> bool {
        self.rows.iter().any(|r| r.changed())
    }

    /// Identifies `row` by its key columns, or by every column when there is no key.
    fn where_for(&self, row: &Row) -> String {
        let keys: Vec<usize> = self.fields.iter().filter(|f| f.key).filter_map(|f| self.columns.iter().position(|c| c == &f.name)).collect();
        let idx: Vec<usize> = if keys.is_empty() { (0..self.columns.len()).collect() } else { keys };
        idx.iter().map(|&i| eq_condition(self.dialect(), &self.columns[i], &row[i])).collect::<Vec<_>>().join(" AND ")
    }

    /// INSERT / UPDATE / DELETE statements for every pending edit.
    pub fn change_statements(&self) -> Vec<String> {
        let d = self.dialect();
        let t = d.quote_ident(&self.table);
        let lim = if self.fields.iter().any(|f| f.key) { "" } else { d.single_row_limit() };
        self.rows
            .iter()
            .filter_map(|r| match (&r.orig, r.deleted) {
                (Some(o), true) => Some(format!("DELETE FROM {} WHERE {}{}", t, self.where_for(o), lim)),
                (Some(o), false) if o != &r.cur => Some(format!("UPDATE {} SET {} WHERE {}{}", t, self.set_list(o, &r.cur), self.where_for(o), lim)),
                (None, false) => Some(self.insert_for(&r.cur)),
                _ => None,
            })
            .collect()
    }

    fn set_list(&self, orig: &Row, cur: &Row) -> String {
        let d = self.dialect();
        (0..self.columns.len()).filter(|&i| orig[i] != cur[i]).map(|i| format!("{} = {}", d.quote_ident(&self.columns[i]), d.literal(&cur[i]))).collect::<Vec<_>>().join(", ")
    }

    /// INSERT of the non-NULL cells of a new row.
    fn insert_for(&self, cur: &Row) -> String {
        let idx: Vec<usize> = (0..self.columns.len()).filter(|&i| cur[i].is_some()).collect();
        if idx.is_empty() {
            return self.dialect().insert_defaults(&self.table);
        }
        let cols: Vec<String> = idx.iter().map(|&i| self.columns[i].clone()).collect();
        let vals: Vec<Cell> = idx.iter().map(|&i| cur[i].clone()).collect();
        insert_row(self.dialect(), &self.table, &cols, &vals)
    }

    /// The row as an INSERT statement, for "Copy as INSERT Statement".
    pub fn insert_sql(&self, r: usize) -> String {
        format!("{};", insert_row(self.dialect(), &self.table, &self.columns, &self.rows[r].cur))
    }

    /// Filters the grid to rows where column `c` equals row `r`'s value.
    pub fn filter_by(&mut self, r: usize, c: usize) {
        self.filter = eq_condition(self.dialect(), &self.columns[c], &self.rows[r].cur[c]);
        self.filter_applied = self.filter.clone();
        self.show_filter = true;
        self.offset = 0;
    }

    pub fn add_row(&mut self) {
        self.rows.push(RowState { orig: None, cur: vec![None; self.columns.len()], deleted: false });
        let r = self.rows.len() - 1;
        self.sel = Some((r, 0));
        self.scroll_to = Some(r);
    }

    /// Deletes a new row outright; marks (or with `toggle`, unmarks) a loaded one.
    pub fn delete_row(&mut self, r: usize, toggle: bool) {
        if self.rows[r].orig.is_none() {
            self.rows.remove(r);
            self.sel = None;
        } else {
            self.rows[r].deleted = !(toggle && self.rows[r].deleted);
        }
    }

    pub fn discard_changes(&mut self) {
        self.editing = None;
        self.rows.retain(|r| r.orig.is_some());
        for r in &mut self.rows {
            r.deleted = false;
            if let Some(o) = &r.orig {
                r.cur = o.clone();
            }
        }
    }

    pub fn start_edit(&mut self, r: usize, c: usize) {
        let text = self.rows[r].cur[c].clone().unwrap_or_default();
        self.editing = Some(CellEdit { row: r, col: c, text, focus: true });
    }

    /// Stores the edited text in the row (an untouched NULL stays NULL).
    pub fn commit_edit(&mut self) {
        let Some(e) = self.editing.take() else { return };
        let Some(row) = self.rows.get_mut(e.row) else { return };
        let unchanged = row.cur[e.col].as_deref() == Some(e.text.as_str()) || (row.cur[e.col].is_none() && e.text.is_empty());
        if !unchanged {
            row.cur[e.col] = Some(e.text);
        }
    }

    /// Clicking a header cycles ascending → descending → unsorted.
    pub fn toggle_order(&mut self, c: usize) {
        let col = self.columns[c].clone();
        self.order = match &self.order {
            Some((oc, false)) if *oc == col => Some((col, true)),
            Some((oc, true)) if *oc == col => None,
            _ => Some((col, false)),
        };
    }

    pub fn total_rows(&self) -> u64 {
        self.total.unwrap_or(self.rows.len() as u64)
    }

    /// (current page, page count), both from 1.
    pub fn pages(&self) -> (usize, usize) {
        let pages = (self.total_rows() as usize).max(1).div_ceil(self.limit);
        (self.offset / self.limit + 1, pages)
    }
}
