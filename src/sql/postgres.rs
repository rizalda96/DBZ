//! PostgreSQL syntax.

use super::dialect::{create_table_head, default_expr, CapRows, Dialect, SqlTemplates, TableChange};
use super::diff::{diff, ColumnDiff};
use super::parts::{ConstraintKind, DesignerCaps};
use super::postgres_parts as parts;
use crate::domain::{FieldDef, IndexDef, TableOptions, TriggerDef};

pub struct PgDialect;

const TYPES: &[&str] = &["integer", "bigint", "smallint", "numeric", "real", "double precision", "varchar", "char", "text", "boolean", "date", "timestamp", "timestamptz", "time", "interval", "json", "jsonb", "uuid", "bytea", "inet"];

static TEMPLATES: SqlTemplates = SqlTemplates {
    new_view: "CREATE VIEW \"new_view\" AS\nSELECT *\nFROM \"table_name\";\n",
    new_function: "CREATE OR REPLACE FUNCTION add_numbers(a integer, b integer)\nRETURNS integer AS $$\nBEGIN\n  RETURN a + b;\nEND;\n$$ LANGUAGE plpgsql;\n",
    new_user: "CREATE ROLE new_user WITH LOGIN;\n",
};

/// The part after the schema, e.g. "orders" for "sales.orders".
fn base_name(name: &str) -> &str {
    name.rsplit('.').next().unwrap_or(name)
}

impl Dialect for PgDialect {
    fn quote_ident(&self, name: &str) -> String {
        let q = |s: &str| format!("\"{}\"", s.replace('"', "\"\""));
        match name.split_once('.') {
            Some((schema, table)) => format!("{}.{}", q(schema), q(table)),
            None => q(name),
        }
    }

    fn escape_str(&self, s: &str) -> String {
        s.replace('\'', "''")
    }

    fn column_def(&self, f: &FieldDef) -> String {
        let ty = if f.auto_inc { serial_type(&f.ty).to_string() } else { f.type_text() };
        let mut s = format!("{} {}", self.quote_ident(&f.name), ty);
        if f.not_null {
            s.push_str(" NOT NULL");
        }
        if !f.default.trim().is_empty() && !f.auto_inc {
            s.push_str(&format!(" DEFAULT {}", default_expr(self, f.default.trim())));
        }
        s
    }

    fn create_table(&self, table: &str, fields: &[FieldDef], comment: &str) -> Vec<String> {
        let t = self.quote_ident(table);
        let mut out = vec![create_table_head(self, table, fields)];
        if !comment.is_empty() {
            out.push(format!("COMMENT ON TABLE {} IS {}", t, self.str_literal(comment)));
        }
        for f in fields.iter().filter(|f| !f.comment.is_empty()) {
            out.push(self.column_comment(&t, &f.name, &f.comment));
        }
        out
    }

    fn alter_table(&self, change: &TableChange) -> Vec<String> {
        let d = diff(change);
        let t = self.quote_ident(change.table);
        let mut out: Vec<String> = d.dropped.iter().map(|n| format!("ALTER TABLE {} DROP COLUMN {}", t, self.quote_ident(n))).collect();
        if d.drop_key {
            let pkey = self.quote_ident(&format!("{}_pkey", base_name(change.table)));
            out.push(format!("ALTER TABLE {} DROP CONSTRAINT IF EXISTS {}", t, pkey));
        }
        for c in &d.columns {
            self.column_statements(&t, c, &mut out);
        }
        if !d.add_key.is_empty() {
            let cols: Vec<String> = d.add_key.iter().map(|k| self.quote_ident(k)).collect();
            out.push(format!("ALTER TABLE {} ADD PRIMARY KEY ({})", t, cols.join(", ")));
        }
        if let Some(c) = d.comment {
            out.push(format!("COMMENT ON TABLE {} IS {}", t, self.str_literal(c)));
        }
        if let Some(n) = d.rename {
            out.push(self.rename_table(change.table, n));
        }
        out
    }

    fn rename_table(&self, table: &str, new_name: &str) -> String {
        format!("ALTER TABLE {} RENAME TO {}", self.quote_ident(table), self.quote_ident(base_name(new_name)))
    }

    fn create_database(&self, name: &str, charset: &str) -> String {
        if charset.is_empty() {
            format!("CREATE DATABASE {}", self.quote_ident(name))
        } else {
            format!("CREATE DATABASE {} ENCODING '{}'", self.quote_ident(name), charset)
        }
    }

    fn begin_transaction(&self) -> &'static str {
        "BEGIN"
    }

    fn read_only_begin(&self) -> &'static str {
        "BEGIN READ ONLY"
    }

    fn read_only_session(&self) -> Option<&'static str> {
        Some("SET SESSION CHARACTERISTICS AS TRANSACTION READ ONLY")
    }

    /// PostgreSQL has no session row cap, so queries are wrapped; SHOW and the like
    /// return a handful of rows and go as they are.
    fn cap_rows(&self, stmt: &str, max: usize) -> CapRows {
        let stmt = stmt.trim().trim_end_matches(';').trim_end();
        let first = stmt.split(|c: char| !c.is_alphanumeric()).next().unwrap_or_default().to_uppercase();
        if ["SELECT", "WITH", "VALUES", "TABLE"].contains(&first.as_str()) {
            CapRows::Wrapped(format!("SELECT * FROM ({}) AS dbz_q LIMIT {}", stmt, max))
        } else {
            CapRows::Wrapped(stmt.to_string())
        }
    }

    fn statement_timeout(&self, secs: u32) -> Option<String> {
        Some(format!("SET statement_timeout = '{}s'", secs))
    }

    /// `public` stays on the path after the chosen schema, so functions from
    /// extensions installed there keep working.
    fn use_schema(&self, schema: &str) -> Option<String> {
        let s = schema.trim();
        if s.is_empty() {
            return None;
        }
        let path = if s == "public" { self.quote_ident(s) } else { format!("{}, public", self.quote_ident(s)) };
        Some(format!("SET search_path TO {}", path))
    }

    fn reset_schema(&self) -> Option<&'static str> {
        Some("RESET search_path")
    }

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
        "integer"
    }

    fn import_field(&self, name: &str) -> FieldDef {
        FieldDef { name: name.to_string(), ty: "text".into(), ..Default::default() }
    }

    fn default_charset(&self) -> &'static str {
        "UTF8"
    }

    fn dump_prologue(&self) -> &'static str {
        ""
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
        parts::drop_index(self, table, name)
    }

    fn drop_constraint(&self, table: &str, _kind: ConstraintKind, name: &str) -> String {
        format!("ALTER TABLE {} DROP CONSTRAINT {}", self.quote_ident(table), self.quote_ident(name))
    }

    fn create_trigger(&self, table: &str, trigger: &TriggerDef) -> Vec<String> {
        parts::create_trigger(self, table, trigger)
    }

    fn drop_trigger(&self, table: &str, name: &str) -> String {
        format!("DROP TRIGGER {} ON {}", self.quote_ident(name), self.quote_ident(table))
    }

    fn table_options(&self, table: &str, old: &TableOptions, new: &TableOptions) -> Vec<String> {
        parts::table_options(self, table, old, new)
    }

    fn constraint_comment(&self, table: &str, name: &str, comment: &str) -> Option<String> {
        Some(format!("COMMENT ON CONSTRAINT {} ON {} IS {}", self.quote_ident(name), self.quote_ident(table), self.str_literal(comment)))
    }

    fn has_matviews(&self) -> bool {
        true
    }
}

fn serial_type(ty: &str) -> &'static str {
    match ty.to_lowercase().as_str() {
        "bigint" | "int8" => "bigserial",
        "smallint" | "int2" => "smallserial",
        _ => "serial",
    }
}

impl PgDialect {
    fn column_comment(&self, table: &str, column: &str, comment: &str) -> String {
        format!("COMMENT ON COLUMN {}.{} IS {}", table, self.quote_ident(column), self.str_literal(comment))
    }

    fn column_statements(&self, t: &str, c: &ColumnDiff, out: &mut Vec<String>) {
        match c {
            ColumnDiff::Added { field, .. } => {
                out.push(format!("ALTER TABLE {} ADD COLUMN {}", t, self.column_def(field)));
                if !field.comment.is_empty() {
                    out.push(self.column_comment(t, &field.name, &field.comment));
                }
            }
            ColumnDiff::Changed { old, field, orig, .. } => out.extend(self.changed_column(t, old, field, orig)),
        }
    }

    fn changed_column(&self, t: &str, o: &FieldDef, n: &FieldDef, orig: &str) -> Vec<String> {
        let mut out = vec![];
        let col = self.quote_ident(&n.name);
        if n.name != orig {
            out.push(format!("ALTER TABLE {} RENAME COLUMN {} TO {}", t, self.quote_ident(orig), col));
        }
        if o.type_text() != n.type_text() {
            let ty = n.type_text();
            out.push(format!("ALTER TABLE {} ALTER COLUMN {} TYPE {} USING {}::{}", t, col, ty, col, ty));
        }
        if o.not_null != n.not_null {
            out.push(format!("ALTER TABLE {} ALTER COLUMN {} {} NOT NULL", t, col, if n.not_null { "SET" } else { "DROP" }));
        }
        if o.default != n.default {
            out.push(self.default_change(t, &col, n.default.trim()));
        }
        if o.comment != n.comment {
            out.push(self.column_comment(t, &n.name, &n.comment));
        }
        out
    }

    fn default_change(&self, t: &str, col: &str, default: &str) -> String {
        if default.is_empty() {
            format!("ALTER TABLE {} ALTER COLUMN {} DROP DEFAULT", t, col)
        } else {
            format!("ALTER TABLE {} ALTER COLUMN {} SET DEFAULT {}", t, col, default_expr(self, default))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::PgDialect;
    use crate::sql::{Dialect, MySqlDialect};

    #[test]
    fn query_schema_goes_first_on_the_search_path() {
        assert_eq!(PgDialect.use_schema("sales").as_deref(), Some("SET search_path TO \"sales\", public"));
        assert_eq!(PgDialect.use_schema("public").as_deref(), Some("SET search_path TO \"public\""));
        assert_eq!(PgDialect.use_schema(" "), None);
        assert_eq!(PgDialect.use_schema("we\"ird").as_deref(), Some("SET search_path TO \"we\"\"ird\", public"));
        assert_eq!((MySqlDialect.use_schema("sales"), MySqlDialect.reset_schema()), (None, None));
    }
}
