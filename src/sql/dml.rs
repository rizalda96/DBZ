//! Row-level statements shared by the data grid, export and dump.

use super::Dialect;
use crate::domain::Cell;

/// `INSERT INTO t (cols) VALUES (vals)` without a trailing semicolon.
pub fn insert_row(d: &dyn Dialect, table: &str, columns: &[String], row: &[Cell]) -> String {
    let cols: Vec<String> = columns.iter().map(|c| d.quote_ident(c)).collect();
    let vals: Vec<String> = row.iter().map(|v| d.literal(v)).collect();
    format!("INSERT INTO {} ({}) VALUES ({})", d.quote_ident(table), cols.join(", "), vals.join(", "))
}

/// `col = value`, or `col IS NULL` for a NULL value.
pub fn eq_condition(d: &dyn Dialect, column: &str, value: &Cell) -> String {
    let col = d.quote_ident(column);
    match value {
        None => format!("{} IS NULL", col),
        v => format!("{} = {}", col, d.literal(v)),
    }
}
