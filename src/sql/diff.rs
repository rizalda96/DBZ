//! What changed between the loaded table and the designer's fields. The dialects
//! turn this into ALTER statements.

use super::TableChange;
use crate::domain::FieldDef;

pub(super) enum ColumnDiff<'a> {
    Added { field: &'a FieldDef, after: Option<&'a str> },
    Changed { old: &'a FieldDef, field: &'a FieldDef, orig: &'a str, after: Option<&'a str> },
}

pub(super) struct TableDiff<'a> {
    pub dropped: Vec<&'a str>,
    pub drop_key: bool,
    pub columns: Vec<ColumnDiff<'a>>,
    /// New primary key columns, when the key changed.
    pub add_key: Vec<&'a str>,
    pub comment: Option<&'a str>,
    pub rename: Option<&'a str>,
}

pub(super) fn diff<'a>(c: &TableChange<'a>) -> TableDiff<'a> {
    let keys_changed = keys_changed(c.old, c.new);
    let old_has_key = c.old.iter().any(|f| f.key);
    let new_keys: Vec<&str> = c.new.iter().filter(|f| f.key && !f.name.trim().is_empty()).map(|f| f.name.as_str()).collect();
    TableDiff {
        dropped: c.old.iter().filter(|o| !c.new.iter().any(|n| n.orig_name.as_deref() == Some(o.name.as_str()))).map(|o| o.name.as_str()).collect(),
        drop_key: keys_changed && old_has_key,
        columns: column_diffs(c.old, c.new),
        add_key: if keys_changed { new_keys } else { vec![] },
        comment: (c.old_comment != c.comment).then_some(c.comment),
        rename: (!c.new_name.is_empty() && c.new_name != c.table).then_some(c.new_name),
    }
}

fn keys_changed(old: &[FieldDef], new: &[FieldDef]) -> bool {
    let old_keys: Vec<&str> = old.iter().filter(|f| f.key).map(|f| f.name.as_str()).collect();
    let new_key_count = new.iter().filter(|f| f.key && !f.name.trim().is_empty()).count();
    let was_key = |f: &FieldDef| f.orig_name.as_deref().map(|o| old_keys.contains(&o));
    old_keys.len() != new_key_count
        || new.iter().filter(|f| f.key).any(|f| !was_key(f).unwrap_or(false))
        || new.iter().any(|f| !f.key && was_key(f).unwrap_or(false))
}

/// Added and changed columns in order, each with the column it follows.
fn column_diffs<'a>(old: &'a [FieldDef], new: &'a [FieldDef]) -> Vec<ColumnDiff<'a>> {
    let mut out = Vec::new();
    let mut prev: Option<&str> = None;
    for field in new.iter().filter(|f| !f.name.trim().is_empty()) {
        let after = prev;
        match &field.orig_name {
            None => out.push(ColumnDiff::Added { field, after }),
            Some(orig) => {
                let Some(o) = old.iter().find(|o| &o.name == orig) else { continue };
                if is_changed(o, field, orig) {
                    out.push(ColumnDiff::Changed { old: o, field, orig, after });
                }
            }
        }
        prev = Some(&field.name);
    }
    out
}

/// Ignores the key flag (handled separately) and compares the rest.
fn is_changed(old: &FieldDef, new: &FieldDef, orig: &str) -> bool {
    let strip = |f: &FieldDef, name: &str| FieldDef { orig_name: None, key: false, name: name.to_string(), ..f.clone() };
    new.name != orig || strip(old, orig) != strip(new, orig)
}
