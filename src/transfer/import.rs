//! Importing CSV / text files into a table.

use super::{parse_csv, read_file};
use crate::db::{DbError, Driver};
use crate::domain::FieldDef;

const BATCH_ROWS: usize = 100;

/// What to import and how. Shared by the dialog and the worker request.
#[derive(Clone, Debug)]
pub struct ImportOptions {
    pub table: String,
    pub path: String,
    pub header: bool,
    pub delim: char,
    /// Create `table` first, with one text column per file column.
    pub create: bool,
}

pub fn import(driver: &mut dyn Driver, db: &str, opts: &ImportOptions) -> Result<String, DbError> {
    let text = read_file(&opts.path)?;
    let mut rows = parse_csv(text.trim_start_matches('\u{feff}'), opts.delim);
    if rows.is_empty() {
        return Err(DbError::Invalid("The file has no rows".into()));
    }
    let width = rows.iter().map(|r| r.len()).max().unwrap_or(0);
    let names: Vec<String> = if opts.header { rows.remove(0) } else { (1..=width).map(|i| format!("column_{}", i)).collect() };
    if opts.create {
        create_table(driver, db, &opts.table, &names)?;
    }
    let targets = match_columns(&driver.columns(db, &opts.table)?, &names, opts.header);
    if targets.is_empty() {
        return Err(DbError::Invalid("None of the file's columns match the table's fields".into()));
    }
    let done = insert_rows(driver, db, &opts.table, &targets, &rows)?;
    Ok(format!("Imported {} record(s) into {} ({} matching field(s))", done, opts.table, targets.len()))
}

fn create_table(driver: &mut dyn Driver, db: &str, table: &str, names: &[String]) -> Result<(), DbError> {
    let dialect = driver.dialect();
    let fields: Vec<FieldDef> = names.iter().map(|n| dialect.import_field(n)).collect();
    for st in dialect.create_table(table, &fields, "") {
        driver.query(db, &st)?;
    }
    Ok(())
}

/// (file column index, table column name) pairs: by name with a header row, by
/// position without one.
fn match_columns(existing: &[FieldDef], names: &[String], header: bool) -> Vec<(usize, String)> {
    names
        .iter()
        .enumerate()
        .filter_map(|(i, n)| {
            let col = if header { existing.iter().find(|f| f.name.eq_ignore_ascii_case(n.trim())) } else { existing.get(i) };
            col.map(|f| (i, f.name.clone()))
        })
        .collect()
}

fn insert_rows(driver: &mut dyn Driver, db: &str, table: &str, targets: &[(usize, String)], rows: &[Vec<String>]) -> Result<usize, DbError> {
    let d = driver.dialect();
    let cols: Vec<String> = targets.iter().map(|(_, n)| d.quote_ident(n)).collect();
    let tuple = |r: &Vec<String>| {
        let vals: Vec<String> = targets.iter().map(|(i, _)| r.get(*i).filter(|v| !v.is_empty()).map(|v| d.str_literal(v)).unwrap_or_else(|| "NULL".into())).collect();
        format!("({})", vals.join(", "))
    };
    let mut done = 0;
    for chunk in rows.chunks(BATCH_ROWS) {
        let tuples: Vec<String> = chunk.iter().map(tuple).collect();
        let sql = format!("INSERT INTO {} ({}) VALUES {}", d.quote_ident(table), cols.join(", "), tuples.join(", "));
        driver.query(db, &sql).map_err(|e| e.context(&format!("Stopped after {} record(s): ", done)))?;
        done += chunk.len();
    }
    Ok(done)
}
