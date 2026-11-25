//! The designer's index, key, check and trigger lists and the table options; no egui.

use super::index_cols_state::{ColsTarget, IndexColsEditor};
use crate::domain::{CheckDef, ExcludeDef, ForeignKeyDef, IndexDef, RuleDef, TableParts, TriggerDef, UniqueDef};
use crate::sql::{DesignerCaps, Dialect};

/// The list tabs between Fields and Options.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PartKind {
    Index,
    ForeignKey,
    Unique,
    Check,
    Exclude,
    Rule,
    Trigger,
}

impl PartKind {
    pub const ALL: [PartKind; 7] = [PartKind::Index, PartKind::ForeignKey, PartKind::Unique, PartKind::Check, PartKind::Exclude, PartKind::Rule, PartKind::Trigger];

    /// Whether the server has this kind of part (its tab shows only then).
    pub fn offered(self, caps: &DesignerCaps) -> bool {
        match self {
            PartKind::Exclude | PartKind::Rule => caps.excludes_and_rules,
            PartKind::Index | PartKind::ForeignKey | PartKind::Unique | PartKind::Check | PartKind::Trigger => true,
        }
    }

    pub fn tab_label(self) -> &'static str {
        match self {
            PartKind::Index => "Indexes",
            PartKind::ForeignKey => "Foreign Keys",
            PartKind::Unique => "Uniques",
            PartKind::Check => "Checks",
            PartKind::Exclude => "Excludes",
            PartKind::Rule => "Rules",
            PartKind::Trigger => "Triggers",
        }
    }

    /// For "Add …" / "Delete …".
    pub fn noun(self) -> &'static str {
        match self {
            PartKind::Index => "Index",
            PartKind::ForeignKey => "Foreign Key",
            PartKind::Unique => "Unique",
            PartKind::Check => "Check",
            PartKind::Exclude => "Exclude",
            PartKind::Rule => "Rule",
            PartKind::Trigger => "Trigger",
        }
    }
}

/// The parts being edited, the parts as loaded, and the selected row of each list.
#[derive(Default)]
pub struct PartsEditor {
    pub parts: TableParts,
    pub orig: TableParts,
    sel: [Option<usize>; PartKind::ALL.len()],
    /// Waiting for the server's parts of an existing table.
    pub loading: bool,
    /// The index fields editor, while it is open.
    pub cols: Option<IndexColsEditor>,
}

impl PartsEditor {
    /// An editor waiting for an existing table's parts.
    pub fn awaiting_server() -> Self {
        PartsEditor { loading: true, ..Default::default() }
    }

    pub fn sel(&self, k: PartKind) -> Option<usize> {
        self.sel[k as usize]
    }

    pub fn sel_mut(&mut self, k: PartKind) -> &mut Option<usize> {
        &mut self.sel[k as usize]
    }

    pub fn len(&self, k: PartKind) -> usize {
        let p = &self.parts;
        match k {
            PartKind::Index => p.indexes.len(),
            PartKind::ForeignKey => p.foreign_keys.len(),
            PartKind::Unique => p.uniques.len(),
            PartKind::Check => p.checks.len(),
            PartKind::Exclude => p.excludes.len(),
            PartKind::Rule => p.rules.len(),
            PartKind::Trigger => p.triggers.len(),
        }
    }

    /// Appends a blank row and selects it.
    pub fn add(&mut self, k: PartKind) {
        let p = &mut self.parts;
        match k {
            PartKind::Index => p.indexes.push(IndexDef::default()),
            PartKind::ForeignKey => p.foreign_keys.push(ForeignKeyDef { on_delete: "NO ACTION".into(), on_update: "NO ACTION".into(), ..Default::default() }),
            PartKind::Unique => p.uniques.push(UniqueDef::default()),
            PartKind::Check => p.checks.push(CheckDef::default()),
            PartKind::Exclude => p.excludes.push(ExcludeDef { method: "gist".into(), ..Default::default() }),
            PartKind::Rule => p.rules.push(RuleDef { event: "INSERT".into(), enabled: true, ..Default::default() }),
            PartKind::Trigger => p.triggers.push(TriggerDef { for_each: "ROW".into(), fires: "AFTER".into(), insert: true, enabled: true, ..Default::default() }),
        }
        self.sel[k as usize] = self.len(k).checked_sub(1);
    }

    /// Removes the selected row; the row below (or the new last one) becomes selected.
    pub fn delete(&mut self, k: PartKind) {
        let Some(i) = self.sel(k).filter(|i| *i < self.len(k)) else { return };
        let p = &mut self.parts;
        match k {
            PartKind::Index => drop_row(&mut p.indexes, i),
            PartKind::ForeignKey => drop_row(&mut p.foreign_keys, i),
            PartKind::Unique => drop_row(&mut p.uniques, i),
            PartKind::Check => drop_row(&mut p.checks, i),
            PartKind::Exclude => drop_row(&mut p.excludes, i),
            PartKind::Rule => drop_row(&mut p.rules, i),
            PartKind::Trigger => drop_row(&mut p.triggers, i),
        }
        self.sel[k as usize] = self.len(k).checked_sub(1).map(|last| i.min(last));
    }

    /// The parts as the server has them; edits start over from here.
    pub fn loaded(&mut self, parts: TableParts) {
        self.orig = parts.clone();
        self.parts = parts;
        self.loading = false;
        // Its index row may be gone now.
        self.cols = None;
        for k in PartKind::ALL {
            let len = self.len(k);
            let sel = self.sel_mut(k);
            *sel = sel.filter(|i| *i < len);
        }
    }
}

impl PartsEditor {
    /// Opens the fields editor on index `i`'s Fields or exclusion `i`'s Elements.
    pub fn open_cols(&mut self, target: ColsTarget) {
        let p = &self.parts;
        let source = match target {
            ColsTarget::Index(i) => p.indexes.get(i).map(|x| (&x.columns, &x.method)),
            ColsTarget::Exclude(i) => p.excludes.get(i).map(|x| (&x.elements, &x.method)),
        };
        self.cols = source.map(|(columns, method)| IndexColsEditor::open(target, columns, method));
    }

    /// Closes the fields editor; with `ok`, writes its rows back where they came from.
    pub fn close_cols(&mut self, ok: bool, d: &dyn Dialect) {
        let Some(ed) = self.cols.take() else { return };
        let Some(text) = ok.then(|| ed.result(d)).flatten() else { return };
        let p = &mut self.parts;
        let slot = match ed.target {
            ColsTarget::Index(i) => p.indexes.get_mut(i).map(|x| &mut x.columns),
            ColsTarget::Exclude(i) => p.excludes.get_mut(i).map(|x| &mut x.elements),
        };
        if let Some(s) = slot {
            *s = text;
        }
    }
}

fn drop_row<T>(items: &mut Vec<T>, i: usize) {
    items.remove(i);
}

#[cfg(test)]
mod tests {
    use super::{ColsTarget, PartKind, PartsEditor};
    use crate::domain::{TableParts, UniqueDef};

    #[test]
    fn add_and_delete_keep_a_valid_selection() {
        let mut e = PartsEditor::default();
        e.add(PartKind::Unique);
        e.add(PartKind::Unique);
        assert_eq!(e.sel(PartKind::Unique), Some(1));
        e.delete(PartKind::Unique);
        assert_eq!((e.len(PartKind::Unique), e.sel(PartKind::Unique)), (1, Some(0)));
        e.delete(PartKind::Unique);
        assert_eq!(e.sel(PartKind::Unique), None);
        assert_eq!(e.sel(PartKind::Index), None);
    }

    #[test]
    fn fields_editor_writes_back_to_the_list_it_came_from() {
        let pg = crate::sql::dialect_for(crate::domain::DbKind::PostgreSql);
        let mut e = PartsEditor::default();
        e.add(PartKind::Index);
        e.add(PartKind::Exclude);
        e.open_cols(ColsTarget::Exclude(0));
        assert_eq!(e.cols.as_ref().map(|c| c.method.as_str()), Some("gist"));
        let row = &mut e.cols.as_mut().expect("just opened").rows[0];
        (row.name, row.operator) = ("room".into(), "=".into());
        e.close_cols(true, pg);
        assert_eq!(e.parts.excludes[0].elements, "\"room\" WITH =");
        assert_eq!(e.parts.indexes[0].columns, "");
        e.open_cols(ColsTarget::Index(0));
        e.cols.as_mut().expect("just opened").rows[0].name = "a".into();
        e.close_cols(false, pg);
        assert!(e.cols.is_none() && e.parts.indexes[0].columns.is_empty());
    }

    #[test]
    fn loading_resets_edits_and_clamps_selection() {
        let mut e = PartsEditor::awaiting_server();
        e.add(PartKind::Unique);
        e.add(PartKind::Unique);
        let server = TableParts { uniques: vec![UniqueDef { name: "u".into(), ..Default::default() }], ..Default::default() }.loaded();
        e.loaded(server.clone());
        assert!(!e.loading);
        assert_eq!(e.parts, server);
        assert_eq!(e.sel(PartKind::Unique), None);
    }
}
