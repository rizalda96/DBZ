//! The index fields editor's rows (no egui): opened on one index's column list (or
//! one exclusion constraint's elements) and written back to it on OK.

use super::state::{move_selected, move_target};
use crate::domain::IndexColumn;
use crate::sql::{index_columns_text, parse_index_columns, Dialect};

/// The row whose columns are being edited.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ColsTarget {
    /// An index's Fields.
    Index(usize),
    /// An exclusion constraint's Elements; each also has an operator.
    Exclude(usize),
}

pub struct IndexColsEditor {
    pub target: ColsTarget,
    /// The index method; the operator classes offered depend on it.
    pub method: String,
    pub rows: Vec<IndexColumn>,
    /// The rows as opened, to tell whether anything changed.
    orig: Vec<IndexColumn>,
    pub sel: Option<usize>,
}

impl IndexColsEditor {
    /// Starts on a column list; an empty one gets one blank row to fill in.
    pub fn open(target: ColsTarget, columns: &str, method: &str) -> Self {
        let orig = parse_index_columns(columns);
        let mut rows = orig.clone();
        if rows.is_empty() {
            rows.push(IndexColumn::default());
        }
        IndexColsEditor { target, method: method.to_string(), rows, orig, sel: Some(0) }
    }

    pub fn add(&mut self) {
        self.rows.push(IndexColumn::default());
        self.sel = Some(self.rows.len() - 1);
    }

    /// Removes the selected row; the row below (or the new last one) becomes selected.
    pub fn remove(&mut self) {
        let Some(i) = self.sel.filter(|i| *i < self.rows.len()) else { return };
        self.rows.remove(i);
        self.sel = self.rows.len().checked_sub(1).map(|last| i.min(last));
    }

    pub fn can_move(&self, delta: isize) -> bool {
        move_target(self.rows.len(), self.sel, delta).is_some()
    }

    pub fn move_sel(&mut self, delta: isize) {
        self.sel = move_selected(&mut self.rows, self.sel, delta);
    }

    /// The new column list; None when the rows are as they were opened, so OK on an
    /// untouched index doesn't rewrite its text (and drop and create it on save).
    pub fn result(&self, d: &dyn Dialect) -> Option<String> {
        let named: Vec<IndexColumn> = self.rows.iter().filter(|c| !c.name.trim().is_empty()).cloned().collect();
        (named != self.orig).then(|| index_columns_text(d, &named))
    }
}

#[cfg(test)]
mod tests {
    use super::{ColsTarget, IndexColsEditor};
    use crate::domain::DbKind;
    use crate::sql::dialect_for;

    #[test]
    fn new_index_starts_with_a_blank_row() {
        let e = IndexColsEditor::open(ColsTarget::Index(0), "", "gin");
        assert_eq!((e.rows.len(), e.sel, e.method.as_str()), (1, Some(0), "gin"));
        assert_eq!(e.result(dialect_for(DbKind::MySql)), None);
    }

    #[test]
    fn untouched_columns_are_not_rewritten() {
        let e = IndexColsEditor::open(ColsTarget::Index(0), "title(10) DESC", "");
        assert_eq!(e.result(dialect_for(DbKind::MySql)), None);
    }

    #[test]
    fn picked_moved_and_removed_rows_are_written_back() {
        let pg = dialect_for(DbKind::PostgreSql);
        let mut e = IndexColsEditor::open(ColsTarget::Index(2), "a, b", "");
        e.add();
        e.rows[2].name = "c".into();
        e.rows[2].order = "DESC".into();
        e.move_sel(-1);
        assert_eq!(e.sel, Some(1));
        assert!(!e.can_move(-2));
        e.sel = Some(0);
        e.remove();
        assert_eq!(e.result(pg).as_deref(), Some("c DESC, b"));
    }
}
