//! SQL Server table edits: column changes, the primary key and MS_Description
//! comments. Defaults and primary keys are named constraints the designer doesn't
//! know the names of, so they are looked up on the server when the statement runs.

use super::diff::{diff, ColumnDiff};
use super::dialect::{Dialect, TableChange};
use super::mssql::{split_name, MssqlDialect};
use crate::domain::FieldDef;

pub(super) fn alter_table(d: &MssqlDialect, change: &TableChange) -> Vec<String> {
    let diff = diff(change);
    let (table, t) = (change.table, d.quote_ident(change.table));
    let mut out = vec![];
    for name in &diff.dropped {
        out.push(drop_default(d, table, name));
        out.push(format!("ALTER TABLE {} DROP COLUMN {}", t, d.quote_ident(name)));
    }
    if diff.drop_key {
        out.push(drop_named(d, table, "SELECT name FROM sys.key_constraints WHERE type = 'PK' AND parent_object_id = OBJECT_ID({obj})".to_string()));
    }
    for c in &diff.columns {
        column_statements(d, table, c, &mut out);
    }
    if !diff.add_key.is_empty() {
        let cols: Vec<String> = diff.add_key.iter().map(|k| d.quote_ident(k)).collect();
        out.push(format!("ALTER TABLE {} ADD PRIMARY KEY ({})", t, cols.join(", ")));
    }
    if let Some(c) = diff.comment {
        out.push(describe(d, table, None, c));
    }
    if let Some(n) = diff.rename {
        out.push(d.rename_table(table, n));
    }
    out
}

fn column_statements(d: &MssqlDialect, table: &str, c: &ColumnDiff, out: &mut Vec<String>) {
    let t = d.quote_ident(table);
    match c {
        ColumnDiff::Added { field, .. } => {
            out.push(format!("ALTER TABLE {} ADD {}", t, d.column_def(field)));
            if !field.comment.is_empty() {
                out.push(describe(d, table, Some(&field.name), &field.comment));
            }
        }
        ColumnDiff::Changed { old, field, orig, .. } => changed_column(d, table, (old, field, orig), out),
    }
}

/// Rename, type and NULL, default and comment of one column. IDENTITY can't be added
/// to or taken off an existing column in SQL Server, so a changed auto-increment
/// tick is left alone.
fn changed_column(d: &MssqlDialect, table: &str, (o, n, orig): (&FieldDef, &FieldDef, &str), out: &mut Vec<String>) {
    let t = d.quote_ident(table);
    if n.name != orig {
        let from = d.str_literal(&format!("{}.{}", t, d.quote_ident(orig)));
        out.push(format!("EXEC sp_rename {}, {}, N'COLUMN'", from, d.str_literal(&n.name)));
    }
    let col = d.quote_ident(&n.name);
    let retyped = o.type_text() != n.type_text() || o.not_null != n.not_null;
    let default_changed = o.default.trim() != n.default.trim();
    // A column with a default can't change type until the default is gone.
    if default_changed || (retyped && !o.default.trim().is_empty()) {
        out.push(drop_default(d, table, &n.name));
    }
    if retyped {
        let null = if n.not_null { "NOT NULL" } else { "NULL" };
        out.push(format!("ALTER TABLE {} ALTER COLUMN {} {} {}", t, col, n.type_text(), null));
    }
    if (default_changed || retyped) && !n.default.trim().is_empty() {
        out.push(format!("ALTER TABLE {} ADD DEFAULT {} FOR {}", t, d.default_value(n.default.trim()), col));
    }
    if o.comment != n.comment {
        out.push(describe(d, table, Some(&n.name), &n.comment));
    }
}

/// Drops the default constraint of `column`, when it has one.
fn drop_default(d: &MssqlDialect, table: &str, column: &str) -> String {
    let lookup = format!(
        "SELECT name FROM sys.default_constraints WHERE parent_object_id = OBJECT_ID({{obj}}) AND parent_column_id = COLUMNPROPERTY(OBJECT_ID({{obj}}), {}, 'ColumnId')",
        d.str_literal(column)
    );
    drop_named(d, table, lookup)
}

/// Drops the constraint of `table` that `lookup` (a query with `{obj}` for the table)
/// names, when there is one.
fn drop_named(d: &MssqlDialect, table: &str, lookup: String) -> String {
    let obj = d.str_literal(&d.quote_ident(table));
    let drop = d.str_literal(&format!("ALTER TABLE {} DROP CONSTRAINT ", d.quote_ident(table)));
    format!("DECLARE @name sysname = ({});\nIF @name IS NOT NULL EXEC ({} + QUOTENAME(@name))", lookup.replace("{obj}", &obj), drop)
}

/// Sets the MS_Description of `table` or one of its columns; an empty `comment`
/// removes it.
pub(super) fn describe(d: &MssqlDialect, table: &str, column: Option<&str>, comment: &str) -> String {
    let (schema, name) = split_name(table);
    let obj = d.str_literal(&d.quote_ident(table));
    let mut args = format!("@name = N'MS_Description', @level0type = N'SCHEMA', @level0name = {}, @level1type = N'TABLE', @level1name = {}", d.str_literal(schema), d.str_literal(name));
    let mut minor = "0".to_string();
    if let Some(c) = column {
        args.push_str(&format!(", @level2type = N'COLUMN', @level2name = {}", d.str_literal(c)));
        minor = format!("COLUMNPROPERTY(OBJECT_ID({}), {}, 'ColumnId')", obj, d.str_literal(c));
    }
    let exists = format!("EXISTS (SELECT 1 FROM sys.extended_properties WHERE class = 1 AND name = N'MS_Description' AND major_id = OBJECT_ID({}) AND minor_id = {})", obj, minor);
    if comment.is_empty() {
        return format!("IF {} EXEC sys.sp_dropextendedproperty {}", exists, args);
    }
    let value = d.str_literal(comment);
    format!("IF {exists} EXEC sys.sp_updateextendedproperty {args}, @value = {value}\nELSE EXEC sys.sp_addextendedproperty {args}, @value = {value}")
}

#[cfg(test)]
mod tests {
    use crate::domain::FieldDef;
    use crate::sql::{Dialect, MssqlDialect, TableChange};

    fn loaded(name: &str, ty: &str) -> FieldDef {
        FieldDef { name: name.into(), ty: ty.into(), ..Default::default() }.loaded()
    }

    fn change<'a>(old: &'a [FieldDef], new: &'a [FieldDef]) -> TableChange<'a> {
        TableChange { table: "sales.orders", old, new, old_comment: "", comment: "", new_name: "" }
    }

    #[test]
    fn renames_and_retypes_a_column() {
        let old = [FieldDef { default: "0".into(), ..loaded("qty", "int") }];
        let new = [FieldDef { name: "amount".into(), ty: "bigint".into(), not_null: true, ..old[0].clone() }];
        let sql = MssqlDialect.alter_table(&change(&old, &new));
        assert_eq!(sql[0], "EXEC sp_rename N'[sales].[orders].[qty]', N'amount', N'COLUMN'");
        assert!(sql[1].starts_with("DECLARE @name sysname = (SELECT name FROM sys.default_constraints"));
        assert!(sql[1].contains("COLUMNPROPERTY(OBJECT_ID(N'[sales].[orders]'), N'amount', 'ColumnId')"));
        assert!(sql[1].ends_with("EXEC (N'ALTER TABLE [sales].[orders] DROP CONSTRAINT ' + QUOTENAME(@name))"));
        assert_eq!(sql[2], "ALTER TABLE [sales].[orders] ALTER COLUMN [amount] bigint NOT NULL");
        assert_eq!(sql[3], "ALTER TABLE [sales].[orders] ADD DEFAULT 0 FOR [amount]");
        assert_eq!(sql.len(), 4);
    }

    #[test]
    fn drops_columns_and_the_key() {
        let old = [FieldDef { key: true, ..loaded("id", "int") }, loaded("note", "nvarchar")];
        let new = [FieldDef { key: false, ..old[0].clone() }];
        let sql = MssqlDialect.alter_table(&change(&old, &new));
        assert!(sql[0].contains("sys.default_constraints"));
        assert_eq!(sql[1], "ALTER TABLE [sales].[orders] DROP COLUMN [note]");
        assert!(sql[2].contains("sys.key_constraints WHERE type = 'PK'"));
        assert_eq!(sql.len(), 3);
    }

    #[test]
    fn comment_updates_or_adds_the_description() {
        let old = [loaded("id", "int")];
        let c = TableChange { comment: "Orders", ..change(&old, &old) };
        let sql = MssqlDialect.alter_table(&c);
        assert_eq!(sql.len(), 1);
        assert!(sql[0].starts_with("IF EXISTS (SELECT 1 FROM sys.extended_properties") && sql[0].contains("minor_id = 0"));
        assert!(sql[0].contains("sp_updateextendedproperty") && sql[0].contains("ELSE EXEC sys.sp_addextendedproperty") && sql[0].contains("@value = N'Orders'"));
    }
}
