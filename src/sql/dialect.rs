//! The `Dialect` strategy: everything about SQL text that differs between servers.

use super::mysql::MySqlDialect;
use super::postgres::PgDialect;
use crate::domain::{Cell, DbKind, FieldDef};

/// Starter scripts offered by the "New View / Function / User" buttons.
pub struct SqlTemplates {
    pub new_view: &'static str,
    pub new_function: &'static str,
    pub new_user: &'static str,
}

/// Everything the designer knows about a table edit.
pub struct TableChange<'a> {
    pub table: &'a str,
    pub old: &'a [FieldDef],
    pub new: &'a [FieldDef],
    pub old_comment: &'a str,
    pub comment: &'a str,
    pub new_name: &'a str,
}

pub trait Dialect: Sync {
    fn quote_ident(&self, name: &str) -> String;
    /// Escapes the inside of a single-quoted string.
    fn escape_str(&self, s: &str) -> String;
    fn column_def(&self, f: &FieldDef) -> String;
    fn create_table(&self, table: &str, fields: &[FieldDef], comment: &str) -> Vec<String>;
    fn alter_table(&self, change: &TableChange) -> Vec<String>;
    fn rename_table(&self, table: &str, new_name: &str) -> String;
    /// `CREATE DATABASE`, with `charset` already cleaned of quotes and spaces.
    fn create_database(&self, name: &str, charset: &str) -> String;
    fn begin_transaction(&self) -> &'static str;
    /// Appended to UPDATE/DELETE on tables without a key, so one edit touches one row.
    fn single_row_limit(&self) -> &'static str;
    fn insert_defaults(&self, table: &str) -> String;
    fn field_types(&self) -> &'static [&'static str];
    fn int_type(&self) -> &'static str;
    /// Column used when an import creates a new table.
    fn import_field(&self, name: &str) -> FieldDef;
    fn default_charset(&self) -> &'static str;
    fn dump_prologue(&self) -> &'static str;
    fn dump_epilogue(&self) -> &'static str;
    fn templates(&self) -> &'static SqlTemplates;

    fn str_literal(&self, s: &str) -> String {
        format!("'{}'", self.escape_str(s))
    }

    fn literal(&self, v: &Cell) -> String {
        match v {
            None => "NULL".to_string(),
            Some(s) => self.str_literal(s),
        }
    }
}

static MYSQL: MySqlDialect = MySqlDialect;
static POSTGRES: PgDialect = PgDialect;

/// The one place that picks a dialect for a server kind. The sample engine speaks MySQL.
pub fn dialect_for(kind: DbKind) -> &'static dyn Dialect {
    match kind {
        DbKind::PostgreSql => &POSTGRES,
        DbKind::MySql | DbKind::MariaDb | DbKind::Sample => &MYSQL,
    }
}

/// A DEFAULT value as SQL: numbers, NULL, booleans, functions and quoted text pass through.
pub(super) fn default_expr(d: &dyn Dialect, value: &str) -> String {
    let upper = value.to_uppercase();
    let passthrough = value.parse::<f64>().is_ok()
        || matches!(upper.as_str(), "NULL" | "TRUE" | "FALSE")
        || upper.starts_with("CURRENT_")
        || upper.ends_with(')')
        || value.starts_with('\'');
    if passthrough {
        value.to_string()
    } else {
        d.str_literal(value)
    }
}

/// `CREATE TABLE t (cols, PRIMARY KEY (...))` shared by both dialects.
pub(super) fn create_table_head(d: &dyn Dialect, table: &str, fields: &[FieldDef]) -> String {
    let named = || fields.iter().filter(|f| !f.name.trim().is_empty());
    let mut parts: Vec<String> = named().map(|f| format!("  {}", d.column_def(f))).collect();
    let keys: Vec<String> = named().filter(|f| f.key).map(|f| d.quote_ident(&f.name)).collect();
    if !keys.is_empty() {
        parts.push(format!("  PRIMARY KEY ({})", keys.join(", ")));
    }
    format!("CREATE TABLE {} (\n{}\n)", d.quote_ident(table), parts.join(",\n"))
}
