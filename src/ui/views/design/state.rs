//! State of a table designer tab.

use super::parts_state::{PartKind, PartsEditor};
use super::ref_state::RefColumns;
use crate::domain::{DbKind, FieldDef, IndexChoices, TableParts};
use crate::sql::{dialect_for, index_column_ref, parts_sql, DesignerCaps, Dialect, PartsChange, TableChange};

const SIZED_TYPES: [&str; 4] = ["varchar", "char", "decimal", "numeric"];

/// The text cells of a field row; focus keeps its column when moving between rows.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum FieldCol {
    Name,
    Type,
    Length,
    Decimals,
    Comment,
}

/// The designer's sub tabs.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DesignSub {
    Fields,
    Part(PartKind),
    Options,
    Comment,
    SqlPreview,
}

impl DesignSub {
    /// In the order the tab strip shows them.
    pub fn all() -> impl Iterator<Item = DesignSub> {
        let parts = PartKind::ALL.into_iter().map(DesignSub::Part);
        std::iter::once(DesignSub::Fields).chain(parts).chain([DesignSub::Options, DesignSub::Comment, DesignSub::SqlPreview])
    }

    /// Whether the server has what this tab edits.
    pub fn offered(self, caps: &DesignerCaps) -> bool {
        match self {
            DesignSub::Part(k) => k.offered(caps),
            DesignSub::Fields | DesignSub::Options | DesignSub::Comment | DesignSub::SqlPreview => true,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            DesignSub::Fields => "Fields",
            DesignSub::Part(k) => k.tab_label(),
            DesignSub::Options => "Options",
            DesignSub::Comment => "Comment",
            DesignSub::SqlPreview => "SQL Preview",
        }
    }
}

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
    pub sub: DesignSub,
    pub sel: Option<usize>,
    /// Cell the grid focuses on its next frame (after Arrow Up/Down).
    pub focus: Option<(usize, FieldCol)>,
    /// Indexes, keys, checks, triggers and options.
    pub parts: PartsEditor,
    /// The server's schemas, collations and operator classes (empty until they arrive).
    pub index_choices: IndexChoices,
    /// Columns of the tables the foreign keys reference.
    pub ref_columns: RefColumns,
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
            sub: DesignSub::Fields,
            sel: Some(0),
            focus: None,
            parts: PartsEditor::default(),
            index_choices: IndexChoices::default(),
            ref_columns: RefColumns::default(),
            loading: false,
            saving: false,
            error: None,
        }
    }

    /// A designer for an existing table; its columns arrive later.
    pub fn existing(id: u64, cid: u64, kind: DbKind, db: String, table: (String, String)) -> Self {
        let (name, comment) = table;
        let base = DesignTab::new_table(id, cid, kind, db);
        let parts = PartsEditor::awaiting_server();
        DesignTab { table: Some(name.clone()), name, fields: vec![], comment: comment.clone(), orig_comment: comment, parts, loading: true, ..base }
    }

    pub fn dialect(&self) -> &'static dyn Dialect {
        dialect_for(self.kind)
    }

    /// The table's named fields as column lists write them (odd names quoted), for
    /// the drop-downs that pick fields.
    pub fn field_refs(&self) -> Vec<String> {
        let d = self.dialect();
        self.fields.iter().filter(|f| !f.name.trim().is_empty()).map(|f| index_column_ref(d, &f.name)).collect()
    }

    /// The statements that would save the design (empty = nothing to save). Parts are
    /// dropped before the column changes and created after them.
    pub fn statements(&self) -> Vec<String> {
        let d = self.dialect();
        let name = self.name.trim();
        let Some(t) = &self.table else {
            if name.is_empty() || self.fields.iter().all(|f| f.name.trim().is_empty()) {
                return vec![];
            }
            let none = TableParts::default();
            let mut out = d.create_table(name, &self.fields, &self.comment);
            out.extend(parts_sql(d, &PartsChange { table: name, new_table: name, old: &none, new: &self.parts.parts }).creates);
            return out;
        };
        let new_table = if name.is_empty() { t.as_str() } else { name };
        let parts = parts_sql(d, &PartsChange { table: t, new_table, old: &self.parts.orig, new: &self.parts.parts });
        let columns = d.alter_table(&TableChange { table: t, old: &self.orig, new: &self.fields, old_comment: &self.orig_comment, comment: &self.comment, new_name: name });
        parts.drops.into_iter().chain(columns).chain(parts.creates).collect()
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
        self.sel = move_selected(&mut self.fields, self.sel, delta);
    }

    pub fn can_move(&self, delta: isize) -> bool {
        move_target(self.fields.len(), self.sel, delta).is_some()
    }

    /// Arrow Down: the next field, or a new blank one past the last. A last field
    /// without a name gets no new one after it, so blank rows don't pile up.
    pub fn select_next(&mut self) {
        let last = self.fields.len().checked_sub(1);
        match (self.sel, last) {
            (Some(i), Some(last)) if i < last => self.sel = Some(i + 1),
            (None, Some(_)) => self.sel = Some(0),
            _ if self.fields.last().is_some_and(|f| f.name.trim().is_empty()) => self.sel = last,
            _ => self.add_field(),
        }
    }

    /// Arrow Up: the previous field.
    pub fn select_prev(&mut self) {
        let prev = self.sel.map_or(0, |i| i.saturating_sub(1));
        self.sel = self.fields.len().checked_sub(1).map(|last| prev.min(last));
    }

    /// Moves the selection with the arrow keys and focuses the same column there.
    pub fn arrow_key(&mut self, down: bool, col: FieldCol) {
        if down {
            self.select_next();
        } else {
            self.select_prev();
        }
        self.focus = self.sel.map(|i| (i, col));
    }
}

/// Where the selected row of a `len`-row list lands when moved by `delta`; None when
/// nothing is selected or it would leave the list.
pub(super) fn move_target(len: usize, sel: Option<usize>, delta: isize) -> Option<usize> {
    sel.and_then(|i| i.checked_add_signed(delta)).filter(|j| *j < len)
}

/// Swaps the selected row with its neighbour `delta` away; returns the new selection.
pub(super) fn move_selected<T>(items: &mut [T], sel: Option<usize>, delta: isize) -> Option<usize> {
    let (Some(i), Some(j)) = (sel, move_target(items.len(), sel, delta)) else { return sel };
    items.swap(i, j);
    Some(j)
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

#[cfg(test)]
mod tests {
    use super::{DesignTab, FieldCol};
    use crate::domain::DbKind;

    fn tab() -> DesignTab {
        DesignTab::new_table(1, 1, DbKind::PostgreSql, "db".into())
    }

    #[test]
    fn arrow_down_on_last_field_adds_one() {
        let mut t = tab();
        t.arrow_key(true, FieldCol::Type);
        assert_eq!(t.fields.len(), 2);
        assert_eq!(t.sel, Some(1));
        assert_eq!(t.focus, Some((1, FieldCol::Type)));
    }

    #[test]
    fn arrow_down_after_blank_last_field_adds_nothing() {
        let mut t = tab();
        t.arrow_key(true, FieldCol::Name);
        t.arrow_key(true, FieldCol::Name);
        assert_eq!(t.fields.len(), 2);
        assert_eq!(t.sel, Some(1));
    }

    #[test]
    fn arrow_keys_move_between_existing_fields() {
        let mut t = tab();
        t.add_field();
        t.fields[1].name = "title".into();
        t.arrow_key(false, FieldCol::Name);
        assert_eq!(t.sel, Some(0));
        t.arrow_key(false, FieldCol::Name);
        assert_eq!(t.sel, Some(0));
        t.arrow_key(true, FieldCol::Name);
        assert_eq!((t.sel, t.fields.len()), (Some(1), 2));
    }
}
