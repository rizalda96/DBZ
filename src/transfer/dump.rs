//! SQL dump of a whole database: structure, optionally data, then views.

use super::write_file;
use crate::db::{DbError, Driver};
use crate::domain::{ConnConfig, TableInfo};
use crate::shared::now_string;
use crate::sql::insert_row;

const RULE: &str = "-- ----------------------------\n";

pub fn dump(driver: &mut dyn Driver, cfg: &ConnConfig, db: &str, path: &str, data: bool) -> Result<String, DbError> {
    let dialect = driver.dialect();
    let tables = driver.tables(db)?;
    let mut out = format!("-- DBZ SQL dump\n-- Connection: {} ({})\n-- Database: {}\n-- Date: {}\n\n", cfg.name, cfg.kind.label(), db, now_string());
    out.push_str(dialect.dump_prologue());
    let mut n_rows = 0;
    let base_tables: Vec<&TableInfo> = tables.iter().filter(|t| !t.is_view).collect();
    for t in &base_tables {
        out.push_str(&table_structure(driver, db, &t.name)?);
        if data {
            n_rows += table_records(driver, db, &t.name, &mut out)?;
        }
    }
    for t in tables.iter().filter(|t| t.is_view) {
        out.push_str(&format!("-- View structure for {}\n", t.name));
        if let Ok(d) = driver.ddl(db, &t.name, true) {
            out.push_str(&d);
            out.push_str(";\n\n");
        }
    }
    out.push_str(dialect.dump_epilogue());
    write_file(path, out)?;
    let records = if data { format!(" and {} record(s)", n_rows) } else { String::new() };
    Ok(format!("Dumped {} table(s){} from {} to {}", base_tables.len(), records, db, path))
}

fn table_structure(driver: &mut dyn Driver, db: &str, table: &str) -> Result<String, DbError> {
    let drop = format!("DROP TABLE IF EXISTS {};\n", driver.dialect().quote_ident(table));
    let ddl = driver.ddl(db, table, false)?;
    Ok(format!("{RULE}-- Table structure for {table}\n{RULE}{drop}{ddl}\n\n"))
}

/// Appends INSERTs for every row; returns how many.
fn table_records(driver: &mut dyn Driver, db: &str, table: &str, out: &mut String) -> Result<usize, DbError> {
    let dialect = driver.dialect();
    let rs = driver.first_set(db, &format!("SELECT * FROM {}", dialect.quote_ident(table)))?;
    if rs.rows.is_empty() {
        return Ok(0);
    }
    out.push_str(&format!("{RULE}-- Records of {table}\n{RULE}"));
    for r in &rs.rows {
        out.push_str(&format!("{};\n", insert_row(dialect, table, &rs.columns, r)));
    }
    out.push('\n');
    Ok(rs.rows.len())
}
