//! The `Dialect` strategy: everything about SQL text that differs between servers.

use super::mssql::MssqlDialect;
use super::mysql::MySqlDialect;
use super::parts::{ConstraintKind, DesignerCaps};
use super::postgres::PgDialect;
use super::split::split_statements;
use crate::domain::{Cell, DbKind, FieldDef, IndexDef, RoutineInfo, TableOptions, TriggerDef};

/// Starter scripts offered by the "New View / Function / User" buttons.
pub struct SqlTemplates {
    pub new_view: &'static str,
    pub new_function: &'static str,
    pub new_user: &'static str,
}

/// How a server shows query plans instead of running the statements.
pub enum Explain {
    /// Each statement is sent with this in front, e.g. `EXPLAIN `.
    Prefix(&'static str),
    /// A session setting turns plans on before the statements and off after them.
    Session { on: &'static str, off: &'static str },
}

/// How a server is kept from sending more than a set number of rows for one statement.
#[derive(Debug, PartialEq)]
pub enum CapRows {
    /// The statement to send instead (or the same one, when it can't be wrapped).
    Wrapped(String),
    /// A session setting turns the cap on before the statement and off after it.
    Session { on: String, off: &'static str },
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
    /// Starts a transaction that the server stops from writing, where it can; the AI's
    /// reads run inside one and always roll back.
    fn read_only_begin(&self) -> &'static str;
    /// Makes every later transaction of the session read-only, so even a statement
    /// that ends the AI's read-only transaction can't write; None where the server has
    /// no such setting.
    fn read_only_session(&self) -> Option<&'static str> {
        None
    }
    /// Limits `stmt` to `max` rows on the server side.
    fn cap_rows(&self, stmt: &str, max: usize) -> CapRows;

    /// Makes the server cancel statements running longer than `secs`; None when the
    /// server has no such session setting.
    fn statement_timeout(&self, _secs: u32) -> Option<String> {
        None
    }

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

    /// Makes unqualified names in a query resolve in `schema` first; None on servers
    /// without schemas.
    fn use_schema(&self, _schema: &str) -> Option<String> {
        None
    }

    /// Undoes [`Dialect::use_schema`] once the query has run, so other work on the
    /// same connection sees the usual names again.
    fn reset_schema(&self) -> Option<&'static str> {
        None
    }

    /// `COMMENT ON CONSTRAINT`, on servers that keep comments on constraints.
    fn constraint_comment(&self, _table: &str, _name: &str, _comment: &str) -> Option<String> {
        None
    }

    /// The pieces of a script that go to the server one at a time.
    fn split_script(&self, sql: &str) -> Vec<String> {
        split_statements(sql)
    }

    /// Paging appended to `SELECT … [ORDER BY …]`; `ordered` says whether the query
    /// already has an ORDER BY.
    fn page_clause(&self, _ordered: bool, limit: usize, offset: usize) -> String {
        format!(" LIMIT {} OFFSET {}", limit, offset)
    }

    /// A query whose only cell is the server's version.
    fn version_query(&self) -> &'static str {
        "SELECT VERSION()"
    }

    /// Starting text of "New Query on This Table", with `table` as the tree names it.
    fn sample_query(&self, table: &str) -> String {
        format!("SELECT *\nFROM {}\nLIMIT 100;\n", table)
    }

    /// A line dumps write between groups of statements, on servers whose scripts are
    /// run in batches (SQL Server's `GO`).
    fn batch_separator(&self) -> Option<&'static str> {
        None
    }

    /// Lets INSERTs fill `table`'s auto-number column (`on`), or stops that again, on
    /// servers that refuse those values otherwise.
    fn identity_insert(&self, _table: &str, _on: bool) -> Option<String> {
        None
    }

    /// "Execute Function": a statement that runs `r`, for the user to fill in the
    /// arguments and run.
    fn routine_call(&self, r: &RoutineInfo) -> String {
        let name = self.quote_ident(&r.name);
        if r.is_procedure() {
            format!("CALL {}();
", name)
        } else {
            format!("SELECT {}();
", name)
        }
    }

    fn explain(&self) -> Explain {
        Explain::Prefix("EXPLAIN ")
    }

    /// Whether the server has materialized views (their tree folder shows only then).
    fn has_matviews(&self) -> bool {
        false
    }

    /// What replaces the leading CREATE of an existing view's definition so that
    /// running it changes the view (see `editable_view`).
    fn redefine_view(&self) -> &'static str {
        "CREATE OR REPLACE"
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
static SQL_SERVER: MssqlDialect = MssqlDialect;

/// The one place that picks a dialect for a server kind.
pub fn dialect_for(kind: DbKind) -> &'static dyn Dialect {
    match kind {
        DbKind::PostgreSql => &POSTGRES,
        DbKind::MySql | DbKind::MariaDb => &MYSQL,
        DbKind::SqlServer => &SQL_SERVER,
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

#[cfg(test)]
mod routine_call_tests {
    use super::Dialect;
    use crate::domain::RoutineInfo;
    use crate::sql::{MssqlDialect, MySqlDialect, PgDialect};

    fn routine(name: &str, schema: &str, kind: &str, returns: &str) -> RoutineInfo {
        RoutineInfo { name: name.into(), schema: schema.into(), kind: kind.into(), returns: returns.into() }
    }

    #[test]
    fn calls_functions_and_procedures_per_server() {
        assert_eq!(MySqlDialect.routine_call(&routine("add_numbers", "", "FUNCTION", "int")), "SELECT `add_numbers`();
");
        assert_eq!(MySqlDialect.routine_call(&routine("load", "", "PROCEDURE", "")), "CALL `load`();
");
        assert_eq!(PgDialect.routine_call(&routine("uuid_generate_v4", "public", "FUNCTION", "uuid")), "SELECT * FROM \"uuid_generate_v4\"();
");
        assert_eq!(PgDialect.routine_call(&routine("etl.load", "etl", "PROCEDURE", "")), "CALL \"etl\".\"load\"();
");
        assert_eq!(MssqlDialect.routine_call(&routine("add_numbers", "dbo", "FUNCTION", "int")), "SELECT [dbo].[add_numbers]();
");
        assert_eq!(MssqlDialect.routine_call(&routine("rows_of", "dbo", "FUNCTION", "TABLE")), "SELECT * FROM [dbo].[rows_of]();
");
        assert_eq!(MssqlDialect.routine_call(&routine("sales.load", "sales", "PROCEDURE", "")), "EXEC [sales].[load];
");
    }

    #[test]
    fn bare_name_drops_only_its_own_schema() {
        assert_eq!(routine("etl.load", "etl", "FUNCTION", "").bare_name(), "load");
        assert_eq!(routine("load", "public", "FUNCTION", "").bare_name(), "load");
        assert_eq!(routine("etlx.load", "etl", "FUNCTION", "").bare_name(), "etlx.load");
    }
}
