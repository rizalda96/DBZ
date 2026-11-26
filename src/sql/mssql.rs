//! SQL Server (T-SQL) syntax.

use super::batches::split_batches;
use super::dialect::{create_table_head, default_expr, Dialect, Explain, SqlTemplates, TableChange};
use super::mssql_alter as alter;
use super::mssql_parts as parts;
use super::parts::{ConstraintKind, DesignerCaps};
use crate::domain::{FieldDef, IndexDef, TableOptions, TriggerDef};

pub struct MssqlDialect;

/// The schema names without a prefix belong to, as `public` is on PostgreSQL.
pub const DEFAULT_SCHEMA: &str = "dbo";

const TYPES: &[&str] = &[
    "int", "bigint", "smallint", "tinyint", "bit", "decimal", "numeric", "money", "float", "real", "nvarchar", "varchar", "nchar", "char", "date", "datetime2", "datetime", "smalldatetime", "time",
    "datetimeoffset", "uniqueidentifier", "varbinary", "binary", "xml",
];

static TEMPLATES: SqlTemplates = SqlTemplates {
    new_view: "CREATE VIEW [dbo].[new_view] AS\nSELECT *\nFROM [dbo].[table_name];\n",
    new_function: "CREATE FUNCTION [dbo].[add_numbers] (@a int, @b int)\nRETURNS int\nAS\nBEGIN\n  RETURN @a + @b;\nEND;\n",
    new_user: "CREATE LOGIN [new_user] WITH PASSWORD = N'Change-me-1';\nCREATE USER [new_user] FOR LOGIN [new_user];\n",
};

/// ("schema", "table") of a name as the tree gives it: "sales.orders", or "orders" in dbo.
pub(super) fn split_name(name: &str) -> (&str, &str) {
    name.split_once('.').unwrap_or((DEFAULT_SCHEMA, name))
}

impl Dialect for MssqlDialect {
    fn quote_ident(&self, name: &str) -> String {
        let q = |s: &str| format!("[{}]", s.replace(']', "]]"));
        match name.split_once('.') {
            Some((schema, table)) => format!("{}.{}", q(schema), q(table)),
            None => q(name),
        }
    }

    fn escape_str(&self, s: &str) -> String {
        s.replace('\'', "''")
    }

    /// Unicode literals, so text outside the database's code page survives.
    fn str_literal(&self, s: &str) -> String {
        format!("N'{}'", self.escape_str(s))
    }

    fn column_def(&self, f: &FieldDef) -> String {
        let mut s = format!("{} {}", self.quote_ident(&f.name), f.type_text());
        if f.auto_inc {
            s.push_str(" IDENTITY(1,1)");
        }
        s.push_str(if f.not_null { " NOT NULL" } else { " NULL" });
        if !f.default.trim().is_empty() {
            s.push_str(&format!(" DEFAULT {}", self.default_value(f.default.trim())));
        }
        s
    }

    fn create_table(&self, table: &str, fields: &[FieldDef], comment: &str) -> Vec<String> {
        let mut out = vec![create_table_head(self, table, fields)];
        if !comment.is_empty() {
            out.push(alter::describe(self, table, None, comment));
        }
        for f in fields.iter().filter(|f| !f.comment.is_empty()) {
            out.push(alter::describe(self, table, Some(&f.name), &f.comment));
        }
        out
    }

    fn alter_table(&self, change: &TableChange) -> Vec<String> {
        alter::alter_table(self, change)
    }

    fn rename_table(&self, table: &str, new_name: &str) -> String {
        format!("EXEC sp_rename {}, {}", self.str_literal(&self.quote_ident(table)), self.str_literal(split_name(new_name).1))
    }

    /// `charset` is a collation here, e.g. `Latin1_General_100_CI_AS_SC_UTF8`.
    fn create_database(&self, name: &str, charset: &str) -> String {
        if charset.is_empty() {
            format!("CREATE DATABASE {}", self.quote_ident(name))
        } else {
            format!("CREATE DATABASE {} COLLATE {}", self.quote_ident(name), charset)
        }
    }

    fn begin_transaction(&self) -> &'static str {
        "BEGIN TRANSACTION"
    }

    /// T-SQL has no LIMIT on UPDATE/DELETE; like PostgreSQL, a keyless edit touches
    /// every identical row.
    fn single_row_limit(&self) -> &'static str {
        ""
    }

    fn insert_defaults(&self, table: &str) -> String {
        format!("INSERT INTO {} DEFAULT VALUES", self.quote_ident(table))
    }

    fn field_types(&self) -> &'static [&'static str] {
        TYPES
    }

    fn int_type(&self) -> &'static str {
        "int"
    }

    fn import_field(&self, name: &str) -> FieldDef {
        FieldDef { name: name.to_string(), ty: "nvarchar".into(), length: "max".into(), ..Default::default() }
    }

    /// The server's default collation is used unless one is typed in.
    fn default_charset(&self) -> &'static str {
        ""
    }

    fn dump_prologue(&self) -> &'static str {
        "SET NOCOUNT ON;\nGO\n\n"
    }

    fn dump_epilogue(&self) -> &'static str {
        ""
    }

    fn templates(&self) -> &'static SqlTemplates {
        &TEMPLATES
    }

    fn designer(&self) -> &'static DesignerCaps {
        &parts::CAPS
    }

    fn create_index(&self, table: &str, index: &IndexDef) -> Vec<String> {
        parts::create_index(self, table, index)
    }

    fn drop_index(&self, table: &str, name: &str) -> String {
        format!("DROP INDEX {} ON {}", self.quote_ident(split_name(name).1), self.quote_ident(table))
    }

    fn drop_constraint(&self, table: &str, _kind: ConstraintKind, name: &str) -> String {
        format!("ALTER TABLE {} DROP CONSTRAINT {}", self.quote_ident(table), self.quote_ident(split_name(name).1))
    }

    fn create_trigger(&self, table: &str, trigger: &TriggerDef) -> Vec<String> {
        parts::create_trigger(self, table, trigger)
    }

    fn drop_trigger(&self, table: &str, name: &str) -> String {
        parts::drop_trigger(self, table, name)
    }

    /// SQL Server has no table options the designer edits.
    fn table_options(&self, _table: &str, _old: &TableOptions, _new: &TableOptions) -> Vec<String> {
        vec![]
    }

    fn split_script(&self, sql: &str) -> Vec<String> {
        split_batches(sql)
    }

    /// OFFSET / FETCH needs an ORDER BY; `(SELECT NULL)` keeps the server's order.
    fn page_clause(&self, ordered: bool, limit: usize, offset: usize) -> String {
        let order = if ordered { "" } else { " ORDER BY (SELECT NULL)" };
        format!("{} OFFSET {} ROWS FETCH NEXT {} ROWS ONLY", order, offset, limit)
    }

    /// ALTER rather than CREATE OR ALTER, which needs SQL Server 2016 SP1.
    fn redefine_view(&self) -> &'static str {
        "ALTER"
    }

    fn version_query(&self) -> &'static str {
        "SELECT CAST(SERVERPROPERTY('ProductVersion') AS nvarchar(128))"
    }

    fn sample_query(&self, table: &str) -> String {
        format!("SELECT TOP 100 *\nFROM {}\n", table)
    }

    /// SHOWPLAN must be alone in its batch; while it is on, nothing else runs.
    fn explain(&self) -> Explain {
        Explain::Session { on: "SET SHOWPLAN_TEXT ON", off: "SET SHOWPLAN_TEXT OFF" }
    }

    fn batch_separator(&self) -> Option<&'static str> {
        Some("GO")
    }

    fn identity_insert(&self, table: &str, on: bool) -> Option<String> {
        Some(format!("SET IDENTITY_INSERT {} {}", self.quote_ident(table), if on { "ON" } else { "OFF" }))
    }
}

impl MssqlDialect {
    /// A DEFAULT value; Unicode literals (`N'…'`) pass through like quoted text does.
    pub(super) fn default_value(&self, value: &str) -> String {
        if value.starts_with("N'") {
            value.to_string()
        } else {
            default_expr(self, value)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::MssqlDialect;
    use crate::domain::FieldDef;
    use crate::sql::Dialect;

    fn field(name: &str, ty: &str, length: &str) -> FieldDef {
        FieldDef { name: name.into(), ty: ty.into(), length: length.into(), ..Default::default() }
    }

    #[test]
    fn quotes_names_and_unicode_text() {
        let d = MssqlDialect;
        assert_eq!(d.quote_ident("sales.or]ders"), "[sales].[or]]ders]");
        assert_eq!(d.quote_ident("orders"), "[orders]");
        assert_eq!(d.str_literal("it's"), "N'it''s'");
        assert_eq!(d.literal(&None), "NULL");
    }

    #[test]
    fn creates_tables_with_identity_and_comments() {
        let id = FieldDef { not_null: true, key: true, auto_inc: true, ..field("id", "int", "") };
        let name = FieldDef { default: "unknown".into(), comment: "Shown name".into(), ..field("name", "nvarchar", "100") };
        let made = FieldDef { default: "getdate()".into(), ..field("made", "datetime2", "") };
        let sql = MssqlDialect.create_table("sales.items", &[id, name, made], "");
        assert_eq!(
            sql[0],
            "CREATE TABLE [sales].[items] (\n  [id] int IDENTITY(1,1) NOT NULL,\n  [name] nvarchar(100) NULL DEFAULT N'unknown',\n  [made] datetime2 NULL DEFAULT getdate(),\n  PRIMARY KEY ([id])\n)"
        );
        assert!(sql[1].contains("sp_addextendedproperty") && sql[1].contains("@level0name = N'sales'") && sql[1].contains("@level2name = N'name'"));
    }

    #[test]
    fn pages_and_samples() {
        let d = MssqlDialect;
        assert_eq!(d.page_clause(false, 1000, 2000), " ORDER BY (SELECT NULL) OFFSET 2000 ROWS FETCH NEXT 1000 ROWS ONLY");
        assert_eq!(d.page_clause(true, 10, 0), " OFFSET 0 ROWS FETCH NEXT 10 ROWS ONLY");
        assert_eq!(d.sample_query("sales.orders"), "SELECT TOP 100 *\nFROM sales.orders\n");
        assert_eq!(d.identity_insert("orders", true).as_deref(), Some("SET IDENTITY_INSERT [orders] ON"));
        assert_eq!(d.rename_table("sales.orders", "sales.orders_old"), "EXEC sp_rename N'[sales].[orders]', N'orders_old'");
    }
}
