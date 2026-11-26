//! Reads a SQL Server table's indexes, constraints and triggers for the designer.
//! Column lists use `FOR XML PATH` rather than STRING_AGG, which SQL Server 2016 lacks.

use super::driver::{find_word, text, Driver};
use super::DbError;
use crate::domain::{CheckDef, ForeignKeyDef, IndexChoices, IndexDef, Row, SchemaName, TableParts, TriggerDef, UniqueDef};

/// Key columns of index `{i}` of object `{o}`, as "a, b DESC" in key order.
const INDEX_COLS: &str = "STUFF((SELECT ', ' + c.name + CASE WHEN ic.is_descending_key = 1 THEN ' DESC' ELSE '' END \
     FROM sys.index_columns ic JOIN sys.columns c ON c.object_id = ic.object_id AND c.column_id = ic.column_id \
     WHERE ic.object_id = {o} AND ic.index_id = {i} AND ic.is_included_column = 0 ORDER BY ic.key_ordinal \
     FOR XML PATH(''), TYPE).value('.', 'nvarchar(max)'), 1, 2, '')";
/// One side of foreign key `fk`'s columns: `{side}` is "parent" or "referenced".
const FK_COLS: &str = "STUFF((SELECT ', ' + c.name FROM sys.foreign_key_columns k \
     JOIN sys.columns c ON c.object_id = k.{side}_object_id AND c.column_id = k.{side}_column_id \
     WHERE k.constraint_object_id = fk.object_id ORDER BY k.constraint_column_id \
     FOR XML PATH(''), TYPE).value('.', 'nvarchar(max)'), 1, 2, '')";
/// Clustered and nonclustered indexes that don't back a primary key or unique constraint
/// (those show as the key fields and under Uniques).
const INDEXES_SQL: &str = "SELECT i.name, i.type_desc, i.is_unique, {cols} FROM sys.indexes i \
     WHERE i.object_id = OBJECT_ID({obj}) AND i.type IN (1, 2) AND i.is_primary_key = 0 AND i.is_unique_constraint = 0 \
     AND i.is_hypothetical = 0 ORDER BY i.name";
const FOREIGN_KEYS_SQL: &str = "SELECT fk.name, {cols}, SCHEMA_NAME(rt.schema_id), rt.name, {ref_cols}, \
     fk.delete_referential_action_desc, fk.update_referential_action_desc \
     FROM sys.foreign_keys fk JOIN sys.objects rt ON rt.object_id = fk.referenced_object_id \
     WHERE fk.parent_object_id = OBJECT_ID({obj}) ORDER BY fk.name";
const UNIQUES_SQL: &str = "SELECT kc.name, {cols} FROM sys.key_constraints kc \
     WHERE kc.parent_object_id = OBJECT_ID({obj}) AND kc.type = 'UQ' ORDER BY kc.name";
const CHECKS_SQL: &str = "SELECT name, definition FROM sys.check_constraints WHERE parent_object_id = OBJECT_ID({obj}) ORDER BY name";
/// EXECUTE AS comes as a principal: none = the caller, -2 = the owner, else a user
/// (SELF is stored as the user who created it). A CLR trigger's method comes from
/// its assembly module, whose class and method names are in a binary collation unlike
/// the assembly's name, so the pieces are joined in the database's collation.
const TRIGGERS_SQL: &str = "SELECT t.name, t.is_instead_of_trigger, OBJECTPROPERTY(t.object_id, 'ExecIsInsertTrigger'), \
     OBJECTPROPERTY(t.object_id, 'ExecIsUpdateTrigger'), OBJECTPROPERTY(t.object_id, 'ExecIsDeleteTrigger'), t.is_disabled, \
     OBJECT_DEFINITION(t.object_id), t.is_not_for_replication, OBJECTPROPERTY(t.object_id, 'IsEncrypted'), \
     COALESCE(m.execute_as_principal_id, am.execute_as_principal_id), USER_NAME(COALESCE(m.execute_as_principal_id, am.execute_as_principal_id)), \
     CASE WHEN t.type = 'TA' THEN QUOTENAME(a.name) COLLATE DATABASE_DEFAULT + '.' + QUOTENAME(am.assembly_class) COLLATE DATABASE_DEFAULT \
     + '.' + QUOTENAME(am.assembly_method) COLLATE DATABASE_DEFAULT END, \
     CAST(ep.value AS nvarchar(4000)) \
     FROM sys.triggers t LEFT JOIN sys.sql_modules m ON m.object_id = t.object_id \
     LEFT JOIN sys.assembly_modules am ON am.object_id = t.object_id LEFT JOIN sys.assemblies a ON a.assembly_id = am.assembly_id \
     LEFT JOIN sys.extended_properties ep ON ep.class = 1 AND ep.major_id = t.object_id AND ep.minor_id = 0 AND ep.name = N'MS_Description' \
     WHERE t.parent_id = OBJECT_ID({obj}) ORDER BY t.name";
/// `execute_as_principal_id` of EXECUTE AS OWNER.
const OWNER_PRINCIPAL: &str = "-2";
/// Tables a foreign key can reference.
const REF_TABLES_SQL: &str = "SELECT s.name, t.name FROM sys.tables t JOIN sys.schemas s ON s.schema_id = t.schema_id \
     WHERE t.is_ms_shipped = 0 ORDER BY s.name, t.name";

/// Everything but the columns; `obj` is the table's `OBJECT_ID` argument as a literal.
pub(super) fn table_parts(d: &mut dyn Driver, db: &str, obj: &str) -> Result<TableParts, DbError> {
    let indexes_sql = INDEXES_SQL.replace("{cols}", &INDEX_COLS.replace("{o}", "i.object_id").replace("{i}", "i.index_id"));
    let fk_sql = FOREIGN_KEYS_SQL.replace("{cols}", &FK_COLS.replace("{side}", "parent")).replace("{ref_cols}", &FK_COLS.replace("{side}", "referenced"));
    let uniques_sql = UNIQUES_SQL.replace("{cols}", &INDEX_COLS.replace("{o}", "kc.parent_object_id").replace("{i}", "kc.unique_index_id"));
    let sql = |q: &str| q.replace("{obj}", obj);
    let parts = TableParts {
        indexes: d.rows(db, &sql(&indexes_sql))?.iter().map(index_def).collect(),
        foreign_keys: d.rows(db, &sql(&fk_sql))?.iter().map(foreign_key_def).collect(),
        uniques: d.rows(db, &sql(&uniques_sql))?.iter().map(|r| UniqueDef { name: text(&r[0]), columns: text(&r[1]), ..Default::default() }).collect(),
        checks: d.rows(db, &sql(CHECKS_SQL))?.iter().map(|r| CheckDef { name: text(&r[0]), expr: text(&r[1]), ..Default::default() }).collect(),
        triggers: d.rows(db, &sql(TRIGGERS_SQL))?.iter().map(trigger_def).collect(),
        // SQL Server has no exclusion constraints or rules, and the designer offers
        // no table options for it.
        excludes: vec![],
        rules: vec![],
        options: Default::default(),
    };
    Ok(parts.loaded())
}

/// The tables a foreign key can point at, by schema.
pub(super) fn index_choices(d: &mut dyn Driver, db: &str) -> Result<IndexChoices, DbError> {
    let tables = d.rows(db, REF_TABLES_SQL)?.iter().map(|r| SchemaName { schema: text(&r[0]), name: text(&r[1]) }).collect();
    Ok(IndexChoices { tables, ..Default::default() })
}

fn index_def(r: &Row) -> IndexDef {
    IndexDef { name: text(&r[0]), method: text(&r[1]), unique: text(&r[2]) == "1", columns: text(&r[3]), ..Default::default() }
}

/// "SET_NULL" → "SET NULL", as `FK_ACTIONS` spells them.
fn fk_action(desc: &str) -> String {
    desc.replace('_', " ")
}

fn foreign_key_def(r: &Row) -> ForeignKeyDef {
    ForeignKeyDef {
        name: text(&r[0]),
        columns: text(&r[1]),
        ref_schema: text(&r[2]),
        ref_table: text(&r[3]),
        ref_columns: text(&r[4]),
        on_delete: fk_action(&text(&r[5])),
        on_update: fk_action(&text(&r[6])),
        ..Default::default()
    }
}

fn trigger_def(r: &Row) -> TriggerDef {
    let definition = text(&r[6]);
    let (head, body) = split_trigger(&definition).unwrap_or_default();
    let clr_method = text(&r[11]);
    let (execute_as, execute_as_user) = match (r[9].as_deref(), text(&r[10])) {
        (None, _) => (String::new(), String::new()),
        (Some(OWNER_PRINCIPAL), _) => ("OWNER".to_string(), String::new()),
        (Some(_), user) => ("USER".to_string(), user),
    };
    TriggerDef {
        name: text(&r[0]),
        fires: if text(&r[1]) == "1" { "INSTEAD OF" } else { "AFTER" }.to_string(),
        insert: text(&r[2]) == "1",
        update: text(&r[3]) == "1",
        delete: text(&r[4]) == "1",
        enabled: text(&r[5]) == "0",
        not_for_replication: text(&r[7]) == "1",
        encrypted: text(&r[8]) == "1",
        execute_as,
        execute_as_user,
        // The catalogue doesn't keep WITH APPEND; the definition does.
        with_append: find_word(head, "APPEND", 0).is_some(),
        external: !clr_method.is_empty(),
        body: if clr_method.is_empty() { body.to_string() } else { clr_method },
        comment: text(&r[12]),
        ..Default::default()
    }
}

/// A trigger's definition split at the `AS` before its statements: the header and the
/// statements, which the designer edits and puts back after `CREATE TRIGGER … AS`.
/// The search starts at the timing word, so a `WITH EXECUTE AS` before it doesn't count.
fn split_trigger(def: &str) -> Option<(&str, &str)> {
    let on = find_word(def, "ON", 0)?;
    let timing = ["AFTER", "FOR", "INSTEAD"].iter().filter_map(|w| find_word(def, w, on)).min()?;
    let as_at = find_word(def, "AS", timing)?;
    Some((&def[..as_at], def[as_at + "AS".len()..].trim()))
}

#[cfg(test)]
mod tests {
    use super::{fk_action, split_trigger};

    #[test]
    fn trigger_body_follows_the_header() {
        let def = "CREATE TRIGGER [dbo].[trg_audit] ON [dbo].[orders]\r\nAFTER INSERT, DELETE\r\nAS\r\nBEGIN\r\n  SET NOCOUNT ON;\r\nEND";
        assert_eq!(split_trigger(def).map(|(_, body)| body), Some("BEGIN\r\n  SET NOCOUNT ON;\r\nEND"));
        let with = "create trigger t on orders with execute as caller for update with append not for replication as update x set a = 1";
        assert_eq!(split_trigger(with), Some(("create trigger t on orders with execute as caller for update with append not for replication ", "update x set a = 1")));
        assert_eq!(split_trigger("garbage"), None);
    }

    #[test]
    fn actions_match_the_designer_list() {
        assert_eq!(fk_action("SET_DEFAULT"), "SET DEFAULT");
        assert_eq!(fk_action("NO_ACTION"), "NO ACTION");
    }
}
