//! State of a table designer tab.

use crate::domain::{DbKind, FieldDef};
use crate::sql::{dialect_for, Dialect, TableChange};

const SIZED_TYPES: [&str; 4] = ["varchar", "char", "decimal", "numeric"];

pub struct DesignTab {
    pub id: u64,
    pub cid: u64,
    pub db: String,
    pub kind: DbKind,
    /// The table being edited; None while designing a new one.
    pub table: Option<String>,
    pub name: String,
    pub fields: Vec<FieldDef>,
    pub orig: Vec<FieldDef>,
    pub comment: String,
    pub orig_comment: String,
    /// 0 Fields, 1 Comment, 2 SQL Preview.
    pub sub: usize,
    pub sel: Option<usize>,
    pub loading: bool,
    pub saving: bool,
    pub error: Option<String>,
}

impl DesignTab {
    /// A designer for a new table, starting with an `id` key column.
    pub fn new_table(id: u64, cid: u64, kind: DbKind, db: String) -> Self {
        let key = FieldDef { name: "id".into(), ty: dialect_for(kind).int_type().into(), not_null: true, key: true, auto_inc: true, ..Default::default() };
        DesignTab {
            id,
            cid,
            db,
            kind,
            table: None,
            name: String::new(),
            fields: vec![key],
            orig: vec![],
            comment: String::new(),
            orig_comment: String::new(),
            sub: 0,
            sel: Some(0),
            loading: false,
            saving: false,
            error: None,
        }
    }

    /// A designer for an existing table; its columns arrive later.
    pub fn existing(id: u64, cid: u64, kind: DbKind, db: String, table: (String, String)) -> Self {
        let (name, comment) = table;
        let base = DesignTab::new_table(id, cid, kind, db);
        DesignTab { table: Some(name.clone()), name, fields: vec![], comment: comment.clone(), orig_comment: comment, loading: true, ..base }
    }

    pub fn dialect(&self) -> &'static dyn Dialect {
        dialect_for(self.kind)
    }

    /// The statements that would save the design (empty = nothing to save).
    pub fn statements(&self) -> Vec<String> {
        let d = self.dialect();
        match &self.table {
            None if self.name.trim().is_empty() || self.fields.iter().all(|f| f.name.trim().is_empty()) => vec![],
            None => d.create_table(self.name.trim(), &self.fields, &self.comment),
            Some(t) => d.alter_table(&TableChange { table: t, old: &self.orig, new: &self.fields, old_comment: &self.orig_comment, comment: &self.comment, new_name: self.name.trim() }),
        }
    }

    fn blank_field() -> FieldDef {
        FieldDef { ty: "varchar".into(), length: "255".into(), ..Default::default() }
    }

    pub fn add_field(&mut self) {
        self.fields.push(Self::blank_field());
        self.sel = Some(self.fields.len() - 1);
    }

    pub fn insert_field(&mut self) {
        let at = self.sel.unwrap_or(0).min(self.fields.len());
        self.fields.insert(at, Self::blank_field());
        self.sel = Some(at);
    }

    pub fn delete_field(&mut self) {
        let Some(i) = self.sel else { return };
        if i < self.fields.len() {
            self.fields.remove(i);
        }
        self.sel = self.fields.len().checked_sub(1).map(|last| i.min(last));
    }

    pub fn toggle_key(&mut self, i: usize) {
        let Some(f) = self.fields.get_mut(i) else { return };
        f.key = !f.key;
        if f.key {
            f.not_null = true;
        }
    }

    /// Moves the selected field by `delta` (-1 up, +1 down).
    pub fn move_field(&mut self, delta: isize) {
        let Some(i) = self.sel else { return };
        let Some(j) = i.checked_add_signed(delta).filter(|j| *j < self.fields.len()) else { return };
        self.fields.swap(i, j);
        self.sel = Some(j);
    }

    pub fn can_move(&self, delta: isize) -> bool {
        self.sel.and_then(|i| i.checked_add_signed(delta)).is_some_and(|j| j < self.fields.len())
    }
}

/// Picks a type from the list, filling or clearing its size.
pub fn set_type(f: &mut FieldDef, ty: &str) {
    f.ty = ty.to_string();
    if !SIZED_TYPES.contains(&ty) {
        f.length.clear();
        f.decimals.clear();
        return;
    }
    if !f.length.is_empty() {
        return;
    }
    if ty.contains("char") {
        f.length = "255".into();
    } else {
        f.length = "10".into();
        f.decimals = "2".into();
    }
}
