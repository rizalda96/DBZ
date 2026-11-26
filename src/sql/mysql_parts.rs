//! MySQL / MariaDB statements for the designer's indexes, triggers and storage options.

use super::dialect::Dialect;
use super::mysql::MySqlDialect;
use super::parts::{column_list, pick, plain_word, ConstraintKind, DesignerCaps, OptionKind, OptionSpec};
use crate::domain::{IndexDef, TableOptions, TriggerDef};

pub(super) static CAPS: DesignerCaps = DesignerCaps {
    index_methods: &["BTREE", "HASH"],
    trigger_timings: &["BEFORE", "AFTER"],
    trigger_one_event: true,
    trigger_extras: false,
    check_no_inherit: false,
    constraint_comments: false,
    trigger_body: "Statement",
    trigger_enable: false,
    trigger_comments: false,
    trigger_options: false,
    index_sub_part: true,
    index_collation_opclass: false,
    index_concurrently: false,
    constraint_options: false,
    excludes_and_rules: false,
    index_params: &[],
    index_placement: false,
    options: &[
        OptionSpec { key: "engine", label: "Engine", kind: OptionKind::Text },
        OptionSpec { key: "charset", label: "Character Set", kind: OptionKind::Text },
        OptionSpec { key: "collation", label: "Collation", kind: OptionKind::Text },
        OptionSpec { key: "auto_increment", label: "Auto Increment", kind: OptionKind::Text },
        OptionSpec { key: "row_format", label: "Row Format", kind: OptionKind::Text },
    ],
};

pub(super) fn create_index(d: &MySqlDialect, table: &str, i: &IndexDef) -> Vec<String> {
    let unique = if i.unique { "UNIQUE " } else { "" };
    let method = pick(&i.method, CAPS.index_methods).map(|m| format!(" USING {}", m)).unwrap_or_default();
    let comment = if i.comment.is_empty() { String::new() } else { format!(" COMMENT {}", d.str_literal(&i.comment)) };
    vec![format!("CREATE {}INDEX {} ON {} ({}){}{}", unique, d.quote_ident(&i.name), d.quote_ident(table), column_list(d, &i.columns), method, comment)]
}

pub(super) fn drop_index(d: &MySqlDialect, table: &str, name: &str) -> String {
    format!("DROP INDEX {} ON {}", d.quote_ident(name), d.quote_ident(table))
}

pub(super) fn drop_constraint(d: &MySqlDialect, table: &str, kind: ConstraintKind, name: &str) -> String {
    // A unique constraint is an index here; DROP CONSTRAINT for checks works on both
    // MySQL 8 and MariaDB, DROP CHECK only on MySQL.
    let what = match kind {
        ConstraintKind::ForeignKey => "FOREIGN KEY",
        ConstraintKind::Unique => "INDEX",
        // MySQL has no exclusion constraints; the designer never makes one there.
        ConstraintKind::Check | ConstraintKind::Exclude => "CONSTRAINT",
    };
    format!("ALTER TABLE {} DROP {} {}", d.quote_ident(table), what, d.quote_ident(name))
}

/// MySQL triggers have one event; the first ticked one is used.
pub(super) fn create_trigger(d: &MySqlDialect, table: &str, t: &TriggerDef) -> Vec<String> {
    let event = [(t.insert, "INSERT"), (t.update, "UPDATE"), (t.delete, "DELETE")].into_iter().find(|(on, _)| *on).map(|(_, e)| e);
    let body = t.body.trim();
    let Some(event) = event.filter(|_| !body.is_empty()) else { return vec![] };
    let timing = pick(&t.fires, CAPS.trigger_timings).unwrap_or("AFTER");
    vec![format!("CREATE TRIGGER {} {} {} ON {} FOR EACH ROW {}", d.quote_ident(&t.name), timing, event, d.quote_ident(table), body)]
}

pub(super) fn drop_trigger(d: &MySqlDialect, name: &str) -> String {
    format!("DROP TRIGGER {}", d.quote_ident(name))
}

pub(super) fn table_options(d: &MySqlDialect, table: &str, old: &TableOptions, new: &TableOptions) -> Vec<String> {
    let get = |m: &TableOptions, k: &str| m.get(k).map(|v| v.trim().to_string()).unwrap_or_default();
    CAPS.options
        .iter()
        .filter(|s| get(old, s.key) != get(new, s.key))
        .filter_map(|s| {
            let now = get(new, s.key);
            let value = plain_word(&now)?.to_string();
            let clause = match s.key {
                "engine" => "ENGINE",
                "charset" => "DEFAULT CHARSET",
                "collation" => "COLLATE",
                "auto_increment" if value.parse::<u64>().is_ok() => "AUTO_INCREMENT",
                "row_format" => "ROW_FORMAT",
                _ => return None,
            };
            Some(format!("ALTER TABLE {} {} = {}", d.quote_ident(table), clause, value))
        })
        .collect()
}
