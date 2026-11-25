//! The `Dialect` strategy: everything about SQL text that differs between servers.

use super::mysql::MySqlDialect;
use super::parts::{ConstraintKind, DesignerCaps};
use super::postgres::PgDialect;
use crate::domain::{Cell, DbKind, FieldDef, IndexDef, TableOptions, TriggerDef};

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

    /// What the designer's Indexes / Foreign Keys / … / Options tabs offer.
    fn designer(&self) -> &'static DesignerCaps;
    fn create_index(&self, table: &str, index: &IndexDef) -> Vec<String>;
    fn drop_index(&self, table: &str, name: &str) -> String;
    fn drop_constraint(&self, table: &str, kind: ConstraintKind, name: &str) -> String;
    /// Empty when the trigger is incomplete (no event or no body).
    fn create_trigger(&self, table: &str, trigger: &TriggerDef) -> Vec<String>;
    fn drop_trigger(&self, table: &str, name: &str) -> String;
    /// Statements that turn the `old` storage options into the `new` ones.
    fn table_options(&self, table: &str, old: &TableOptions, new: &TableOptions) -> Vec<String>;

    /// `COMMENT ON CONSTRAINT`, on servers that keep comments on constraints.
    fn constraint_comment(&self, _table: &str, _name: &str, _comment: &str) -> Option<String> {
        None
    }

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

/// The one place that picks a dialect for a server kind.
pub fn dialect_for(kind: DbKind) -> &'static dyn Dialect {
    match kind {
        DbKind::PostgreSql => &POSTGRES,
        DbKind::MySql | DbKind::MariaDb => &MYSQL,
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
