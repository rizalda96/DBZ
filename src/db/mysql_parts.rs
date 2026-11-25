//! Reads a MySQL / MariaDB table's indexes, constraints, triggers and options for the designer.

use super::driver::{text, Driver};
use super::DbError;
use crate::domain::{CheckDef, ForeignKeyDef, IndexDef, Row, TableOptions, TableParts, TriggerDef, UniqueDef};

/// Plain (non-unique) indexes; unique ones are listed as unique constraints.
const INDEXES_SQL: &str = "SELECT INDEX_NAME, COLUMN_NAME, COLLATION, SUB_PART, INDEX_TYPE, INDEX_COMMENT FROM information_schema.STATISTICS \
     WHERE TABLE_SCHEMA = {db} AND TABLE_NAME = {table} AND NON_UNIQUE = 1 ORDER BY INDEX_NAME, SEQ_IN_INDEX";
const FOREIGN_KEYS_SQL: &str = "SELECT k.CONSTRAINT_NAME, k.COLUMN_NAME, k.REFERENCED_TABLE_SCHEMA, k.REFERENCED_TABLE_NAME, k.REFERENCED_COLUMN_NAME, r.DELETE_RULE, r.UPDATE_RULE \
     FROM information_schema.KEY_COLUMN_USAGE k JOIN information_schema.REFERENTIAL_CONSTRAINTS r \
     ON r.CONSTRAINT_SCHEMA = k.CONSTRAINT_SCHEMA AND r.CONSTRAINT_NAME = k.CONSTRAINT_NAME AND r.TABLE_NAME = k.TABLE_NAME \
     WHERE k.TABLE_SCHEMA = {db} AND k.TABLE_NAME = {table} AND k.REFERENCED_TABLE_NAME IS NOT NULL ORDER BY k.CONSTRAINT_NAME, k.ORDINAL_POSITION";
const UNIQUES_SQL: &str = "SELECT c.CONSTRAINT_NAME, k.COLUMN_NAME FROM information_schema.TABLE_CONSTRAINTS c JOIN information_schema.KEY_COLUMN_USAGE k \
     ON k.CONSTRAINT_SCHEMA = c.CONSTRAINT_SCHEMA AND k.CONSTRAINT_NAME = c.CONSTRAINT_NAME AND k.TABLE_NAME = c.TABLE_NAME \
     WHERE c.TABLE_SCHEMA = {db} AND c.TABLE_NAME = {table} AND c.CONSTRAINT_TYPE = 'UNIQUE' ORDER BY c.CONSTRAINT_NAME, k.ORDINAL_POSITION";
const CHECKS_SQL: &str = "SELECT cc.CONSTRAINT_NAME, cc.CHECK_CLAUSE FROM information_schema.CHECK_CONSTRAINTS cc JOIN information_schema.TABLE_CONSTRAINTS tc \
     ON tc.CONSTRAINT_SCHEMA = cc.CONSTRAINT_SCHEMA AND tc.CONSTRAINT_NAME = cc.CONSTRAINT_NAME \
     WHERE tc.TABLE_SCHEMA = {db} AND tc.TABLE_NAME = {table} AND tc.CONSTRAINT_TYPE = 'CHECK' ORDER BY cc.CONSTRAINT_NAME";
const TRIGGERS_SQL: &str = "SELECT TRIGGER_NAME, ACTION_TIMING, EVENT_MANIPULATION, ACTION_STATEMENT FROM information_schema.TRIGGERS \
     WHERE EVENT_OBJECT_SCHEMA = {db} AND EVENT_OBJECT_TABLE = {table} ORDER BY TRIGGER_NAME";
const OPTIONS_SQL: &str = "SELECT t.ENGINE, c.CHARACTER_SET_NAME, t.TABLE_COLLATION, t.AUTO_INCREMENT, t.ROW_FORMAT FROM information_schema.TABLES t \
     LEFT JOIN information_schema.COLLATIONS c ON c.COLLATION_NAME = t.TABLE_COLLATION WHERE t.TABLE_SCHEMA = {db} AND t.TABLE_NAME = {table}";

/// Everything but the columns; `db` and `table` are SQL string literals.
pub(super) fn table_parts(d: &mut dyn Driver, db: &str, (db_lit, table_lit): (&str, &str)) -> Result<TableParts, DbError> {
    let sql = |q: &str| q.replace("{db}", db_lit).replace("{table}", table_lit);
    let parts = TableParts {
        indexes: group(&d.rows(db, &sql(INDEXES_SQL))?).into_iter().map(|(r, cols)| index_def(r, cols)).collect(),
        foreign_keys: foreign_keys(&d.rows(db, &sql(FOREIGN_KEYS_SQL))?),
        uniques: group(&d.rows(db, &sql(UNIQUES_SQL))?).into_iter().map(|(r, cols)| UniqueDef { name: text(&r[0]), columns: cols.join(", "), ..Default::default() }).collect(),
        // CHECK constraints only exist from MySQL 8.0.16 / MariaDB 10.2; older servers
        // lack the catalogue table, and then the table simply has none.
        checks: d.rows(db, &sql(CHECKS_SQL)).map(|rows| rows.iter().map(|r| CheckDef { name: text(&r[0]), expr: text(&r[1]), ..Default::default() }).collect()).unwrap_or_default(),
        triggers: d.rows(db, &sql(TRIGGERS_SQL))?.iter().map(trigger_def).collect(),
        options: d.rows(db, &sql(OPTIONS_SQL))?.first().map(options).unwrap_or_default(),
        // MySQL has neither exclusion constraints nor rules.
        excludes: vec![],
        rules: vec![],
    };
    Ok(parts.loaded())
}

/// Rows of one object per column (ordered by name) → (first row, column texts).
fn group(rows: &[Row]) -> Vec<(&Row, Vec<String>)> {
    let mut out: Vec<(&Row, Vec<String>)> = vec![];
    for r in rows {
        let col = column_text(r);
        match out.last_mut() {
            Some((first, cols)) if text(&first[0]) == text(&r[0]) => cols.push(col),
            _ => out.push((r, vec![col])),
        }
    }
    out
}

/// "name", "name(10)" for a prefix index, "name DESC" for a descending one.
fn column_text(r: &Row) -> String {
    let name = text(&r[1]);
    let prefix = r.get(3).map(text).filter(|s| !s.is_empty()).map(|n| format!("({})", n)).unwrap_or_default();
    let desc = if r.get(2).map(text).as_deref() == Some("D") { " DESC" } else { "" };
    format!("{}{}{}", name, prefix, desc)
}

fn index_def(r: &Row, cols: Vec<String>) -> IndexDef {
    IndexDef { name: text(&r[0]), columns: cols.join(", "), method: text(&r[4]), unique: false, comment: text(&r[5]), ..Default::default() }
}

/// Foreign key rows come one per column pair.
fn foreign_keys(rows: &[Row]) -> Vec<ForeignKeyDef> {
    let mut out: Vec<ForeignKeyDef> = vec![];
    for r in rows {
        let (col, ref_col) = (text(&r[1]), text(&r[4]));
        match out.last_mut() {
            Some(fk) if fk.name == text(&r[0]) => {
                fk.columns = format!("{}, {}", fk.columns, col);
                fk.ref_columns = format!("{}, {}", fk.ref_columns, ref_col);
            }
            _ => out.push(ForeignKeyDef {
                name: text(&r[0]),
                columns: col,
                ref_schema: text(&r[2]),
                ref_table: text(&r[3]),
                ref_columns: ref_col,
                on_delete: text(&r[5]),
                on_update: text(&r[6]),
                ..Default::default()
            }),
        }
    }
    out
}

fn trigger_def(r: &Row) -> TriggerDef {
    let event = text(&r[2]);
    TriggerDef {
        name: text(&r[0]),
        for_each: "ROW".into(),
        fires: text(&r[1]),
        insert: event == "INSERT",
        update: event == "UPDATE",
        delete: event == "DELETE",
        enabled: true,
        body: text(&r[3]),
        ..Default::default()
    }
}

fn options(r: &Row) -> TableOptions {
    let keys = ["engine", "charset", "collation", "auto_increment", "row_format"];
    keys.iter().zip(r.iter()).map(|(k, v)| (k.to_string(), text(v))).collect()
}
