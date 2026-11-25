//! PostgreSQL statements for the designer's indexes, triggers and storage options.

use super::dialect::Dialect;
use super::parts::{column_list, pick, DesignerCaps, OptionKind, OptionSpec};
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
    excludes_and_rules: true,
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
    let mut out = vec![format!("CREATE {}INDEX {}{} ON {}{} ({})", unique, concurrently, d.quote_ident(&i.name), d.quote_ident(table), method, column_list(d, &i.columns))];
    if !i.comment.is_empty() {
        out.push(format!("COMMENT ON INDEX {} IS {}", d.quote_ident(&in_schema(table, &i.name)), d.str_literal(&i.comment)));
    }
    out
}

pub(super) fn drop_index(d: &PgDialect, table: &str, name: &str) -> String {
    format!("DROP INDEX {}", d.quote_ident(&in_schema(table, name)))
}

pub(super) fn create_trigger(d: &PgDialect, table: &str, t: &TriggerDef) -> Vec<String> {
    let events = events(d, t);
    let body = t.body.trim();
    if events.is_empty() || body.is_empty() {
        return vec![];
    }
    let timing = pick(&t.fires, CAPS.trigger_timings).unwrap_or("AFTER");
    let level = if t.for_each.trim().eq_ignore_ascii_case("STATEMENT") { "STATEMENT" } else { "ROW" };
    let call = if body.ends_with(')') { body.to_string() } else { format!("{}()", body) };
    let (name, qt) = (d.quote_ident(&t.name), d.quote_ident(table));
    let mut out = vec![format!("CREATE TRIGGER {} {} {} ON {} FOR EACH {} EXECUTE FUNCTION {}", name, timing, events.join(" OR "), qt, level, call)];
    if !t.enabled {
        out.push(format!("ALTER TABLE {} DISABLE TRIGGER {}", qt, name));
    }
    if !t.comment.is_empty() {
        out.push(format!("COMMENT ON TRIGGER {} ON {} IS {}", name, qt, d.str_literal(&t.comment)));
    }
    out
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
