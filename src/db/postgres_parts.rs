//! Reads a PostgreSQL table's indexes, constraints, triggers and options for the designer.

use super::driver::{find_word, number, text, Driver};
use super::DbError;
use crate::domain::{CheckDef, Collation, ExcludeDef, ForeignKeyDef, IndexChoices, IndexDef, OpClass, Row, RuleDef, SchemaName, TableOptions, TableParts, TriggerDef, UniqueDef};

/// Indexes that don't back a constraint (those show under their own tab).
const INDEXES_SQL: &str = "SELECT i.relname, am.amname, ix.indisunique, obj_description(i.oid, 'pg_class'), \
     (SELECT string_agg(pg_get_indexdef(ix.indexrelid, k, true) || CASE WHEN (ix.indoption[k - 1] & 1) = 1 THEN ' DESC' ELSE '' END, ', ' ORDER BY k) \
      FROM generate_series(1, ix.indnkeyatts) k), \
     COALESCE(array_to_string(i.reloptions, ','), ''), COALESCE(ts.spcname, ''), COALESCE(pg_get_expr(ix.indpred, ix.indrelid), '') \
     FROM pg_index ix JOIN pg_class i ON i.oid = ix.indexrelid JOIN pg_am am ON am.oid = i.relam \
     LEFT JOIN pg_tablespace ts ON ts.oid = i.reltablespace \
     WHERE ix.indrelid = {reg}::regclass AND NOT ix.indisprimary \
     AND NOT EXISTS (SELECT 1 FROM pg_constraint c WHERE c.conindid = ix.indexrelid AND c.conrelid = ix.indrelid) \
     ORDER BY i.relname";
const TABLESPACES_SQL: &str = "SELECT spcname FROM pg_tablespace ORDER BY spcname";
/// Tables (plain and partitioned) a foreign key can point at, outside the system schemas.
const REF_TABLES_SQL: &str = "SELECT n.nspname, c.relname FROM pg_class c JOIN pg_namespace n ON n.oid = c.relnamespace \
     WHERE c.relkind IN ('r','p') AND n.nspname NOT IN ('pg_catalog','information_schema') AND n.nspname NOT LIKE 'pg_toast%' ORDER BY 1, 2";
/// `conkey` / `confkey` as "a, b" in key order.
const KEY_COLS: &str = "(SELECT string_agg(a.attname, ', ' ORDER BY k.n) FROM unnest(c.{key}) WITH ORDINALITY k(att, n) \
     JOIN pg_attribute a ON a.attrelid = c.{rel} AND a.attnum = k.att)";
const FOREIGN_KEYS_SQL: &str = "SELECT c.conname, {cols}, rn.nspname, rc.relname, {ref_cols}, c.confdeltype, c.confupdtype, obj_description(c.oid, 'pg_constraint'), \
     c.confmatchtype, c.condeferrable, c.condeferred \
     FROM pg_constraint c JOIN pg_class rc ON rc.oid = c.confrelid JOIN pg_namespace rn ON rn.oid = rc.relnamespace \
     WHERE c.conrelid = {reg}::regclass AND c.contype = 'f' ORDER BY c.conname";
/// Tablespace and fill factor come from the index behind the constraint.
const UNIQUES_SQL: &str = "SELECT c.conname, {cols}, obj_description(c.oid, 'pg_constraint'), COALESCE(ts.spcname, ''), \
     COALESCE((SELECT substring(o from 'fillfactor=(.*)') FROM unnest(i.reloptions) o WHERE o LIKE 'fillfactor=%'), ''), c.condeferrable, c.condeferred \
     FROM pg_constraint c LEFT JOIN pg_class i ON i.oid = c.conindid LEFT JOIN pg_tablespace ts ON ts.oid = i.reltablespace \
     WHERE c.conrelid = {reg}::regclass AND c.contype = 'u' ORDER BY c.conname";
const CHECKS_SQL: &str = "SELECT c.conname, pg_get_expr(c.conbin, c.conrelid), c.connoinherit, obj_description(c.oid, 'pg_constraint') \
     FROM pg_constraint c WHERE c.conrelid = {reg}::regclass AND c.contype = 'c' ORDER BY c.conname";
/// The function comes from `tgfoid`; its arguments and the WHEN condition are read
/// back from the trigger's definition, as SQL text.
const TRIGGERS_SQL: &str = "SELECT t.tgname, t.tgtype, t.tgenabled <> 'D', fn.nspname, p.proname, \
     COALESCE(substring(pg_get_triggerdef(t.oid) from 'EXECUTE (?:FUNCTION|PROCEDURE) [^(]+\\((.*)\\)$'), ''), \
     (SELECT string_agg(a.attname, ', ') FROM unnest(t.tgattr::int2[]) k(att) JOIN pg_attribute a ON a.attrelid = t.tgrelid AND a.attnum = k.att), \
     obj_description(t.oid, 'pg_trigger'), \
     COALESCE(substring(pg_get_triggerdef(t.oid) from 'WHEN \\((.*)\\) EXECUTE (?:FUNCTION|PROCEDURE) '), ''), \
     t.tgconstraint <> 0, t.tgdeferrable, t.tginitdeferred, COALESCE(rn.nspname, ''), COALESCE(rc.relname, '') \
     FROM pg_trigger t JOIN pg_proc p ON p.oid = t.tgfoid JOIN pg_namespace fn ON fn.oid = p.pronamespace \
     LEFT JOIN pg_class rc ON rc.oid = t.tgconstrrelid LEFT JOIN pg_namespace rn ON rn.oid = rc.relnamespace \
     WHERE t.tgrelid = {reg}::regclass AND NOT t.tgisinternal ORDER BY t.tgname";
/// Roles that can own objects; the built-in `pg_*` ones can't log in and aren't listed.
const ROLES_SQL: &str = "SELECT rolname FROM pg_roles WHERE rolname !~ '^pg_' ORDER BY 1";
/// Functions a trigger can execute, outside the system schemas.
const TRIGGER_FUNCTIONS_SQL: &str = "SELECT n.nspname, p.proname FROM pg_proc p JOIN pg_namespace n ON n.oid = p.pronamespace \
     WHERE p.prorettype = 'trigger'::regtype AND n.nspname NOT IN ('pg_catalog', 'information_schema') ORDER BY 1, 2";
/// Elements come as "col WITH op" in key order; the predicate is the backing index's.
const EXCLUDES_SQL: &str = "SELECT c.conname, am.amname, \
     (SELECT string_agg(pg_get_indexdef(c.conindid, k, true) || ' WITH ' || (SELECT o.oprname FROM pg_operator o WHERE o.oid = c.conexclop[k]), ', ' ORDER BY k) \
      FROM generate_subscripts(c.conexclop, 1) k), \
     pg_get_expr(ix.indpred, ix.indrelid, true), obj_description(c.oid, 'pg_constraint') \
     FROM pg_constraint c JOIN pg_class i ON i.oid = c.conindid JOIN pg_am am ON am.oid = i.relam JOIN pg_index ix ON ix.indexrelid = c.conindid \
     WHERE c.conrelid = {reg}::regclass AND c.contype = 'x' ORDER BY c.conname";
/// `_RETURN` is the rule that makes a view; it isn't the user's.
const RULES_SQL: &str = "SELECT r.rulename, r.ev_type, r.is_instead, r.ev_enabled <> 'D', pg_get_ruledef(r.oid), obj_description(r.oid, 'pg_rewrite') \
     FROM pg_rewrite r WHERE r.ev_class = {reg}::regclass AND r.rulename <> '_RETURN' ORDER BY r.rulename";
const OPTIONS_SQL: &str = "SELECT c.relpersistence = 'u', pg_get_userbyid(c.relowner), COALESCE(ts.spcname, ''), \
     COALESCE((SELECT substring(o from 'fillfactor=(.*)') FROM unnest(c.reloptions) o WHERE o LIKE 'fillfactor=%'), '') \
     FROM pg_class c LEFT JOIN pg_tablespace ts ON ts.oid = c.reltablespace WHERE c.oid = {reg}::regclass";

/// Every schema but the per-session temporary ones.
const ALL_SCHEMAS_SQL: &str = "SELECT nspname FROM pg_namespace WHERE nspname !~ '^pg_(toast_)?temp_' ORDER BY nspname";
/// Collations usable with this database's encoding.
const COLLATIONS_SQL: &str = "SELECT n.nspname, c.collname FROM pg_collation c JOIN pg_namespace n ON n.oid = c.collnamespace \
     WHERE c.collencoding IN (-1, (SELECT encoding FROM pg_database WHERE datname = current_database())) ORDER BY n.nspname, c.collname";
const OPCLASSES_SQL: &str = "SELECT n.nspname, o.opcname, am.amname FROM pg_opclass o JOIN pg_namespace n ON n.oid = o.opcnamespace \
     JOIN pg_am am ON am.oid = o.opcmethod ORDER BY n.nspname, o.opcname";
/// Operators an exclusion constraint can compare with: binary, returning boolean.
const OPERATORS_SQL: &str = "SELECT DISTINCT oprname FROM pg_operator WHERE oprkind = 'b' AND oprresult = 'boolean'::regtype ORDER BY 1";

// pg_trigger.tgtype bits.
const TRIGGER_ROW: i64 = 1;
const TRIGGER_BEFORE: i64 = 2;
const TRIGGER_INSERT: i64 = 4;
const TRIGGER_DELETE: i64 = 8;
const TRIGGER_UPDATE: i64 = 16;
const TRIGGER_TRUNCATE: i64 = 32;
const TRIGGER_INSTEAD: i64 = 64;

/// Everything but the columns of `reg` (a `'"schema"."table"'` literal).
pub(super) fn table_parts(d: &mut dyn Driver, db: &str, reg: &str) -> Result<TableParts, DbError> {
    let sql = |q: &str| {
        let cols = KEY_COLS.replace("{key}", "conkey").replace("{rel}", "conrelid");
        let ref_cols = KEY_COLS.replace("{key}", "confkey").replace("{rel}", "confrelid");
        q.replace("{cols}", &cols).replace("{ref_cols}", &ref_cols).replace("{reg}", reg)
    };
    let parts = TableParts {
        indexes: d.rows(db, &sql(INDEXES_SQL))?.iter().map(index_def).collect(),
        foreign_keys: d.rows(db, &sql(FOREIGN_KEYS_SQL))?.iter().map(foreign_key_def).collect(),
        uniques: d.rows(db, &sql(UNIQUES_SQL))?.iter().map(unique_def).collect(),
        checks: d.rows(db, &sql(CHECKS_SQL))?.iter().map(check_def).collect(),
        excludes: d.rows(db, &sql(EXCLUDES_SQL))?.iter().map(exclude_def).collect(),
        rules: d.rows(db, &sql(RULES_SQL))?.iter().map(rule_def).collect(),
        triggers: d.rows(db, &sql(TRIGGERS_SQL))?.iter().map(trigger_def).collect(),
        options: d.rows(db, &sql(OPTIONS_SQL))?.first().map(options).unwrap_or_default(),
    };
    Ok(parts.loaded())
}

/// What the index fields editor offers in its drop-downs.
pub(super) fn index_choices(d: &mut dyn Driver, db: &str) -> Result<IndexChoices, DbError> {
    Ok(IndexChoices {
        schemas: d.rows(db, ALL_SCHEMAS_SQL)?.iter().map(|r| text(&r[0])).collect(),
        collations: d.rows(db, COLLATIONS_SQL)?.iter().map(|r| Collation { schema: text(&r[0]), name: text(&r[1]) }).collect(),
        opclasses: d.rows(db, OPCLASSES_SQL)?.iter().map(|r| OpClass { schema: text(&r[0]), name: text(&r[1]), method: text(&r[2]) }).collect(),
        operators: d.rows(db, OPERATORS_SQL)?.iter().map(|r| text(&r[0])).collect(),
        tablespaces: d.rows(db, TABLESPACES_SQL)?.iter().map(|r| text(&r[0])).collect(),
        tables: schema_names(&d.rows(db, REF_TABLES_SQL)?),
        trigger_functions: schema_names(&d.rows(db, TRIGGER_FUNCTIONS_SQL)?),
        roles: d.rows(db, ROLES_SQL)?.iter().map(|r| text(&r[0])).collect(),
    })
}

fn index_def(r: &Row) -> IndexDef {
    IndexDef {
        name: text(&r[0]),
        method: text(&r[1]),
        unique: text(&r[2]) == "t",
        comment: text(&r[3]),
        columns: text(&r[4]),
        storage: storage_params(&text(&r[5])),
        tablespace: text(&r[6]),
        predicate: text(&r[7]),
        ..Default::default()
    }
}

/// "fillfactor=70,buffering=on" (reloptions) → {fillfactor: 70, buffering: on}.
fn storage_params(list: &str) -> TableOptions {
    list.split(',').filter_map(|kv| kv.split_once('=')).map(|(k, v)| (k.trim().to_string(), v.trim().to_string())).collect()
}

fn exclude_def(r: &Row) -> ExcludeDef {
    ExcludeDef { name: text(&r[0]), method: text(&r[1]), elements: text(&r[2]), predicate: text(&r[3]), comment: text(&r[4]), orig_name: None }
}

fn rule_def(r: &Row) -> RuleDef {
    let event = match text(&r[1]).as_str() {
        "1" => "SELECT",
        "2" => "UPDATE",
        "4" => "DELETE",
        _ => "INSERT",
    };
    let (condition, body) = split_rule(&text(&r[4]));
    RuleDef { name: text(&r[0]), event: event.into(), instead: text(&r[2]) == "t", condition, body, enabled: text(&r[3]) == "t", comment: text(&r[5]), orig_name: None }
}

/// The WHERE condition and the commands of a `pg_get_ruledef` text:
/// "CREATE RULE r AS ON INSERT TO t WHERE (…) DO INSTEAD NOTHING;" → ("(…)", "").
fn split_rule(def: &str) -> (String, String) {
    let Some(to) = find_word(def, "TO", 0) else { return Default::default() };
    let Some(do_at) = find_word(def, "DO", to) else { return Default::default() };
    let head = &def[to..do_at];
    let condition = find_word(head, "WHERE", 0).map(|w| head[w + "WHERE".len()..].trim().to_string()).unwrap_or_default();
    let mut body = def[do_at + "DO".len()..].trim();
    for mode in ["INSTEAD", "ALSO"] {
        body = strip_word(body, mode);
    }
    let body = body.trim_end_matches(';').trim();
    let body = if body.eq_ignore_ascii_case("NOTHING") { "" } else { body };
    (condition, body.to_string())
}

fn strip_word<'a>(s: &'a str, word: &str) -> &'a str {
    match s.get(..word.len()) {
        Some(head) if head.eq_ignore_ascii_case(word) && s[word.len()..].starts_with(char::is_whitespace) => s[word.len()..].trim_start(),
        _ => s,
    }
}

fn fk_action(code: &str) -> String {
    match code {
        "r" => "RESTRICT",
        "c" => "CASCADE",
        "n" => "SET NULL",
        "d" => "SET DEFAULT",
        _ => "NO ACTION",
    }
    .to_string()
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
        comment: text(&r[7]),
        match_full: text(&r[8]) == "f",
        deferrable: text(&r[9]) == "t",
        deferred: text(&r[10]) == "t",
        orig_name: None,
    }
}

/// (schema, name) rows.
fn schema_names(rows: &[Row]) -> Vec<SchemaName> {
    rows.iter().map(|r| SchemaName { schema: text(&r[0]), name: text(&r[1]) }).collect()
}

fn unique_def(r: &Row) -> UniqueDef {
    UniqueDef {
        name: text(&r[0]),
        columns: text(&r[1]),
        comment: text(&r[2]),
        tablespace: text(&r[3]),
        fill_factor: text(&r[4]),
        deferrable: text(&r[5]) == "t",
        deferred: text(&r[6]) == "t",
        orig_name: None,
    }
}

fn check_def(r: &Row) -> CheckDef {
    CheckDef { name: text(&r[0]), expr: text(&r[1]), no_inherit: text(&r[2]) == "t", comment: text(&r[3]), orig_name: None }
}

fn trigger_def(r: &Row) -> TriggerDef {
    let ty: i64 = number(&r[1]).unwrap_or(0);
    let has = |bit: i64| ty & bit != 0;
    let fires = if has(TRIGGER_INSTEAD) {
        "INSTEAD OF"
    } else if has(TRIGGER_BEFORE) {
        "BEFORE"
    } else {
        "AFTER"
    };
    TriggerDef {
        name: text(&r[0]),
        for_each: if has(TRIGGER_ROW) { "ROW" } else { "STATEMENT" }.to_string(),
        fires: fires.to_string(),
        insert: has(TRIGGER_INSERT),
        update: has(TRIGGER_UPDATE),
        delete: has(TRIGGER_DELETE),
        truncate: has(TRIGGER_TRUNCATE),
        enabled: text(&r[2]) == "t",
        function_schema: text(&r[3]),
        function: text(&r[4]),
        arguments: text(&r[5]),
        update_of: text(&r[6]),
        comment: text(&r[7]),
        when: text(&r[8]),
        constraint: text(&r[9]) == "t",
        deferrable: text(&r[10]) == "t",
        deferred: text(&r[11]) == "t",
        ref_schema: text(&r[12]),
        ref_table: text(&r[13]),
        ..Default::default()
    }
}

fn options(r: &Row) -> TableOptions {
    let unlogged = if text(&r[0]) == "t" { "true" } else { "false" };
    [("unlogged", unlogged.to_string()), ("owner", text(&r[1])), ("tablespace", text(&r[2])), ("fillfactor", text(&r[3]))].into_iter().map(|(k, v)| (k.to_string(), v)).collect()
}

#[cfg(test)]
mod tests {
    use super::split_rule;

    #[test]
    fn rule_definitions_split_into_condition_and_commands() {
        let pretty = "CREATE RULE r AS\n    ON INSERT TO public.t\n   WHERE (new.a < 0) DO INSTEAD  NOTHING;";
        assert_eq!(split_rule(pretty), ("(new.a < 0)".to_string(), String::new()));
        let two = "CREATE RULE log AS ON UPDATE TO t DO ( INSERT INTO log VALUES (old.id); NOTIFY t; );";
        assert_eq!(split_rule(two), (String::new(), "( INSERT INTO log VALUES (old.id); NOTIFY t; )".to_string()));
        let also = "CREATE RULE x AS ON DELETE TO t DO ALSO DELETE FROM u WHERE u.id = old.id;";
        assert_eq!(split_rule(also), (String::new(), "DELETE FROM u WHERE u.id = old.id".to_string()));
    }
}
