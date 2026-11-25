//! PostgreSQL statements for the designer's indexes, triggers and storage options.

use super::dialect::Dialect;
use super::index_columns::qualify;
use super::parts::{column_list, deferrable, pick, DesignerCaps, IndexParamSpec, OptionKind, OptionSpec};
use super::postgres::PgDialect;
use crate::domain::{IndexDef, TableOptions, TriggerDef};

pub(super) static CAPS: DesignerCaps = DesignerCaps {
    index_methods: &["btree", "hash", "gist", "gin", "spgist", "brin"],
    trigger_timings: &["BEFORE", "AFTER", "INSTEAD OF"],
    trigger_one_event: false,
    trigger_extras: true,
    check_no_inherit: true,
    constraint_comments: true,
    trigger_body: "Function",
    index_sub_part: false,
    index_collation_opclass: true,
    index_concurrently: true,
    constraint_options: true,
    excludes_and_rules: true,
    index_params: &[
        IndexParamSpec { key: "fillfactor", label: "Fill Factor (%)", values: &[], methods: &["btree", "hash", "gist", "spgist"] },
        IndexParamSpec { key: "buffering", label: "Buffering", values: &["auto", "on", "off"], methods: &["gist"] },
        IndexParamSpec { key: "fastupdate", label: "Fast Update", values: &["on", "off"], methods: &["gin"] },
        IndexParamSpec { key: "gin_pending_list_limit", label: "Pending List Limit", values: &[], methods: &["gin"] },
        IndexParamSpec { key: "pages_per_range", label: "Pages Per Range", values: &[], methods: &["brin"] },
    ],
    index_placement: true,
    options: &[
        OptionSpec { key: "unlogged", label: "Unlogged", kind: OptionKind::Bool },
        OptionSpec { key: "owner", label: "Owner", kind: OptionKind::Text },
        OptionSpec { key: "tablespace", label: "Tablespace", kind: OptionKind::Text },
        OptionSpec { key: "fillfactor", label: "Fill Factor (%)", kind: OptionKind::Text },
    ],
};

/// "sales.orders" + "ix" → "sales.ix": an index lives in its table's schema.
fn in_schema(table: &str, name: &str) -> String {
    match table.split_once('.') {
        Some((schema, _)) => format!("{}.{}", schema, name),
        None => name.to_string(),
    }
}

pub(super) fn create_index(d: &PgDialect, table: &str, i: &IndexDef) -> Vec<String> {
    let unique = if i.unique { "UNIQUE " } else { "" };
    let concurrently = if i.concurrently { "CONCURRENTLY " } else { "" };
    let method = pick(&i.method, CAPS.index_methods).map(|m| format!(" USING {}", m)).unwrap_or_default();
    let head = format!("CREATE {}INDEX {}{} ON {}{} ({})", unique, concurrently, d.quote_ident(&i.name), d.quote_ident(table), method, column_list(d, &i.columns));
    let mut out = vec![head + &index_tail(d, i)];
    if !i.comment.is_empty() {
        out.push(format!("COMMENT ON INDEX {} IS {}", d.quote_ident(&in_schema(table, &i.name)), d.str_literal(&i.comment)));
    }
    out
}

/// ` WITH (…) TABLESPACE … WHERE …` after the column list. Parameters the index
/// method doesn't take, or with values it can't have, are left out.
fn index_tail(d: &PgDialect, i: &IndexDef) -> String {
    let params: Vec<String> = CAPS
        .index_params
        .iter()
        .filter(|p| p.applies(&i.method))
        .filter_map(|p| i.storage.get(p.key).and_then(|v| p.valid(v)).map(|v| format!("{} = {}", p.key, v.to_lowercase())))
        .collect();
    let mut tail = String::new();
    if !params.is_empty() {
        tail.push_str(&format!(" WITH ({})", params.join(", ")));
    }
    if !i.tablespace.trim().is_empty() {
        tail.push_str(&format!(" TABLESPACE {}", d.quote_ident(i.tablespace.trim())));
    }
    if !i.predicate.trim().is_empty() {
        tail.push_str(&format!(" WHERE {}", i.predicate.trim()));
    }
    tail
}

pub(super) fn drop_index(d: &PgDialect, table: &str, name: &str) -> String {
    format!("DROP INDEX {}", d.quote_ident(&in_schema(table, name)))
}

/// `CREATE [CONSTRAINT] TRIGGER … EXECUTE FUNCTION f(args)`, then DISABLE and COMMENT
/// when needed. Empty while it has no event or no function.
pub(super) fn create_trigger(d: &PgDialect, table: &str, t: &TriggerDef) -> Vec<String> {
    let events = events(d, t);
    let function = t.function.trim();
    if events.is_empty() || function.is_empty() {
        return vec![];
    }
    let (name, qt) = (d.quote_ident(&t.name), d.quote_ident(table));
    let mut s = if t.constraint {
        // Constraint triggers always fire AFTER, once per row.
        format!("CREATE CONSTRAINT TRIGGER {} AFTER {} ON {}{} FOR EACH ROW", name, events.join(" OR "), qt, constraint_clauses(d, t))
    } else {
        let timing = pick(&t.fires, CAPS.trigger_timings).unwrap_or("AFTER");
        let level = if t.for_each.trim().eq_ignore_ascii_case("STATEMENT") { "STATEMENT" } else { "ROW" };
        format!("CREATE TRIGGER {} {} {} ON {} FOR EACH {}", name, timing, events.join(" OR "), qt, level)
    };
    if !t.when.trim().is_empty() {
        s.push_str(&format!(" WHEN ({})", t.when.trim()));
    }
    s.push_str(&format!(" EXECUTE FUNCTION {}({})", qualify(d, &t.function_schema, function), t.arguments.trim()));
    let mut out = vec![s];
    if !t.enabled {
        out.push(format!("ALTER TABLE {} DISABLE TRIGGER {}", qt, name));
    }
    if !t.comment.is_empty() {
        out.push(format!("COMMENT ON TRIGGER {} ON {} IS {}", name, qt, d.str_literal(&t.comment)));
    }
    out
}

/// ` FROM table` and the DEFERRABLE clause of a constraint trigger.
fn constraint_clauses(d: &PgDialect, t: &TriggerDef) -> String {
    let from = match t.ref_table.trim() {
        "" => String::new(),
        table => format!(" FROM {}", qualify(d, &t.ref_schema, table)),
    };
    format!("{}{}", from, deferrable(t.deferrable, t.deferred))
}

fn events(d: &PgDialect, t: &TriggerDef) -> Vec<String> {
    let update = match t.update_of.trim() {
        "" => "UPDATE".to_string(),
        cols => format!("UPDATE OF {}", column_list(d, cols)),
    };
    [(t.insert, "INSERT".to_string()), (t.update, update), (t.delete, "DELETE".to_string()), (t.truncate, "TRUNCATE".to_string())].into_iter().filter(|(on, _)| *on).map(|(_, e)| e).collect()
}

pub(super) fn table_options(d: &PgDialect, table: &str, old: &TableOptions, new: &TableOptions) -> Vec<String> {
    let t = d.quote_ident(table);
    let mut out = vec![];
    for spec in CAPS.options {
        let get = |m: &TableOptions| {
            let v = m.get(spec.key).map(|v| v.trim().to_string()).unwrap_or_default();
            if spec.kind == OptionKind::Bool && v.is_empty() { "false".to_string() } else { v }
        };
        let (was, now) = (get(old), get(new));
        if was != now {
            out.extend(option_action(d, spec.key, &now).map(|a| format!("ALTER TABLE {} {}", t, a)));
        }
    }
    out
}

fn option_action(d: &PgDialect, key: &str, v: &str) -> Option<String> {
    match key {
        "unlogged" => Some(if v == "true" { "SET UNLOGGED" } else { "SET LOGGED" }.to_string()),
        "owner" => (!v.is_empty()).then(|| format!("OWNER TO {}", d.quote_ident(v))),
        "tablespace" => Some(format!("SET TABLESPACE {}", d.quote_ident(if v.is_empty() { "pg_default" } else { v }))),
        "fillfactor" if v.is_empty() => Some("RESET (fillfactor)".to_string()),
        "fillfactor" => v.parse::<u8>().ok().map(|n| format!("SET (fillfactor = {})", n)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::create_trigger;
    use crate::domain::TriggerDef;
    use crate::sql::PgDialect;

    fn trigger() -> TriggerDef {
        TriggerDef {
            name: "trg".into(),
            for_each: "ROW".into(),
            fires: "BEFORE".into(),
            insert: true,
            update: true,
            update_of: "id_site".into(),
            enabled: true,
            function_schema: "public".into(),
            function: "insert_data_pooling".into(),
            ..Default::default()
        }
    }

    #[test]
    fn trigger_with_when_function_and_arguments() {
        let t = TriggerDef { when: "new.id_site IS NOT NULL".into(), arguments: "'pool', 3".into(), ..trigger() };
        assert_eq!(
            create_trigger(&PgDialect, "t", &t),
            ["CREATE TRIGGER \"trg\" BEFORE INSERT OR UPDATE OF \"id_site\" ON \"t\" FOR EACH ROW WHEN (new.id_site IS NOT NULL) EXECUTE FUNCTION \"public\".\"insert_data_pooling\"('pool', 3)"]
        );
        assert!(create_trigger(&PgDialect, "t", &TriggerDef { function: " ".into(), ..trigger() }).is_empty());
    }

    #[test]
    fn constraint_trigger_fires_after_each_row_from_its_table() {
        let t = TriggerDef { constraint: true, deferrable: true, deferred: true, ref_schema: "public".into(), ref_table: "master_site".into(), for_each: "STATEMENT".into(), ..trigger() };
        assert_eq!(
            create_trigger(&PgDialect, "t", &t),
            ["CREATE CONSTRAINT TRIGGER \"trg\" AFTER INSERT OR UPDATE OF \"id_site\" ON \"t\" FROM \"public\".\"master_site\" DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION \"public\".\"insert_data_pooling\"()"]
        );
    }
}
