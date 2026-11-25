//! MySQL / MariaDB syntax (also used by the built-in sample engine).

use super::dialect::{create_table_head, default_expr, Dialect, SqlTemplates, TableChange};
use super::diff::{diff, ColumnDiff};
use super::mysql_parts as parts;
use super::parts::{ConstraintKind, DesignerCaps};
use crate::domain::{FieldDef, IndexDef, TableOptions, TriggerDef};

pub struct MySqlDialect;

const TYPES: &[&str] = &["int", "bigint", "smallint", "tinyint", "decimal", "float", "double", "varchar", "char", "text", "mediumtext", "longtext", "date", "datetime", "timestamp", "time", "year", "json", "blob", "enum", "bit"];

static TEMPLATES: SqlTemplates = SqlTemplates {
    new_view: "CREATE VIEW `new_view` AS\nSELECT *\nFROM `table_name`;\n",
    new_function: "CREATE FUNCTION add_numbers(a INT, b INT)\nRETURNS INT DETERMINISTIC\nRETURN a + b;\n",
    new_user: "CREATE USER 'new_user'@'%';\nGRANT SELECT ON *.* TO 'new_user'@'%';\n",
};

impl Dialect for MySqlDialect {
    fn quote_ident(&self, name: &str) -> String {
        format!("`{}`", name.replace('`', "``"))
    }

    fn escape_str(&self, s: &str) -> String {
        s.replace('\'', "''").replace('\\', "\\\\")
    }

    fn column_def(&self, f: &FieldDef) -> String {
        let mut s = format!("{} {}", self.quote_ident(&f.name), f.type_text());
        if f.not_null {
            s.push_str(" NOT NULL");
        }
        if !f.default.trim().is_empty() {
            s.push_str(&format!(" DEFAULT {}", default_expr(self, f.default.trim())));
        }
        if f.auto_inc {
            s.push_str(" AUTO_INCREMENT");
        }
        if !f.comment.is_empty() {
            s.push_str(&format!(" COMMENT {}", self.str_literal(&f.comment)));
        }
        s
    }

    fn create_table(&self, table: &str, fields: &[FieldDef], comment: &str) -> Vec<String> {
        let mut sql = create_table_head(self, table, fields);
        if !comment.is_empty() {
            sql.push_str(&format!(" COMMENT = {}", self.str_literal(comment)));
        }
        vec![sql]
    }

    fn alter_table(&self, change: &TableChange) -> Vec<String> {
        let d = diff(change);
        let mut actions: Vec<String> = d.dropped.iter().map(|n| format!("DROP COLUMN {}", self.quote_ident(n))).collect();
        if d.drop_key {
            actions.push("DROP PRIMARY KEY".into());
        }
        actions.extend(d.columns.iter().map(|c| self.column_action(c)));
        if !d.add_key.is_empty() {
            let cols: Vec<String> = d.add_key.iter().map(|k| self.quote_ident(k)).collect();
            actions.push(format!("ADD PRIMARY KEY ({})", cols.join(", ")));
        }
        if let Some(c) = d.comment {
            actions.push(format!("COMMENT = {}", self.str_literal(c)));
        }
        let mut out = vec![];
        if !actions.is_empty() {
            out.push(format!("ALTER TABLE {}\n  {}", self.quote_ident(change.table), actions.join(",\n  ")));
        }
        if let Some(n) = d.rename {
            out.push(self.rename_table(change.table, n));
        }
        out
    }

    fn rename_table(&self, table: &str, new_name: &str) -> String {
        format!("ALTER TABLE {} RENAME TO {}", self.quote_ident(table), self.quote_ident(new_name))
    }

    fn create_database(&self, name: &str, charset: &str) -> String {
        if charset.is_empty() {
            format!("CREATE DATABASE {}", self.quote_ident(name))
        } else {
            format!("CREATE DATABASE {} CHARACTER SET {}", self.quote_ident(name), charset)
        }
    }

    fn begin_transaction(&self) -> &'static str {
        "START TRANSACTION"
    }

    fn single_row_limit(&self) -> &'static str {
        " LIMIT 1"
    }

    fn insert_defaults(&self, table: &str) -> String {
        format!("INSERT INTO {} () VALUES ()", self.quote_ident(table))
    }

    fn field_types(&self) -> &'static [&'static str] {
        TYPES
    }

    fn int_type(&self) -> &'static str {
        "int"
    }

    fn import_field(&self, name: &str) -> FieldDef {
        FieldDef { name: name.to_string(), ty: "varchar".into(), length: "255".into(), ..Default::default() }
    }

    fn default_charset(&self) -> &'static str {
        "utf8mb4"
    }

    fn dump_prologue(&self) -> &'static str {
        "SET FOREIGN_KEY_CHECKS = 0;\n\n"
    }

    fn dump_epilogue(&self) -> &'static str {
        "SET FOREIGN_KEY_CHECKS = 1;\n"
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
        parts::drop_index(self, table, name)
    }

    fn drop_constraint(&self, table: &str, kind: ConstraintKind, name: &str) -> String {
        parts::drop_constraint(self, table, kind, name)
    }

    fn create_trigger(&self, table: &str, trigger: &TriggerDef) -> Vec<String> {
        parts::create_trigger(self, table, trigger)
    }

    fn drop_trigger(&self, _table: &str, name: &str) -> String {
        parts::drop_trigger(self, name)
    }

    fn table_options(&self, table: &str, old: &TableOptions, new: &TableOptions) -> Vec<String> {
        parts::table_options(self, table, old, new)
    }
}

impl MySqlDialect {
    fn column_action(&self, c: &ColumnDiff) -> String {
        match c {
            ColumnDiff::Added { field, after } => format!("ADD COLUMN {}{}", self.column_def(field), self.position(*after)),
            ColumnDiff::Changed { field, orig, after, .. } => format!("CHANGE COLUMN {} {}{}", self.quote_ident(orig), self.column_def(field), self.position(*after)),
        }
    }

    fn position(&self, after: Option<&str>) -> String {
        match after {
            None => " FIRST".into(),
            Some(p) => format!(" AFTER {}", self.quote_ident(p)),
        }
    }
}
