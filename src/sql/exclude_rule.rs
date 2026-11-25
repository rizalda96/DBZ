//! Statements for exclusion constraints and rewrite rules. Only PostgreSQL has them, so
//! there is one spelling and it lives here; other servers' designers don't offer them.

use super::dialect::Dialect;
use super::parts::{column_list, pick};
use crate::domain::{ExcludeDef, RuleDef};

/// Events a table's rule can be ON. ON SELECT rules only exist on views.
pub const RULE_EVENTS: [&str; 3] = ["INSERT", "UPDATE", "DELETE"];

/// `ALTER TABLE … ADD CONSTRAINT … EXCLUDE USING m (elements) WHERE (predicate)`.
pub(super) fn add_exclude(d: &dyn Dialect, table: &str, e: &ExcludeDef) -> String {
    let method = pick(&e.method, d.designer().index_methods).map(|m| format!(" USING {}", m)).unwrap_or_default();
    let predicate = match e.predicate.trim() {
        "" => String::new(),
        p => format!(" WHERE ({})", p),
    };
    format!("ALTER TABLE {} ADD CONSTRAINT {} EXCLUDE{} ({}){}", d.quote_ident(table), d.quote_ident(&e.name), method, column_list(d, &e.elements), predicate)
}

/// CREATE RULE, then DISABLE RULE and COMMENT ON RULE when needed. Empty when the event
/// isn't one of [`RULE_EVENTS`].
pub(super) fn create_rule(d: &dyn Dialect, table: &str, r: &RuleDef) -> Vec<String> {
    let Some(event) = pick(&r.event, &RULE_EVENTS) else { return vec![] };
    let (name, qt) = (d.quote_ident(&r.name), d.quote_ident(table));
    let condition = match r.condition.trim() {
        "" => String::new(),
        c => format!(" WHERE {}", c),
    };
    let mode = if r.instead { "INSTEAD" } else { "ALSO" };
    let mut out = vec![format!("CREATE RULE {} AS ON {} TO {}{} DO {} {}", name, event, qt, condition, mode, rule_body(&r.body))];
    if !r.enabled {
        out.push(format!("ALTER TABLE {} DISABLE RULE {}", qt, name));
    }
    if !r.comment.is_empty() {
        out.push(format!("COMMENT ON RULE {} ON {} IS {}", name, qt, d.str_literal(&r.comment)));
    }
    out
}

pub(super) fn drop_rule(d: &dyn Dialect, table: &str, name: &str) -> String {
    format!("DROP RULE {} ON {}", d.quote_ident(name), d.quote_ident(table))
}

/// NOTHING when empty; several commands go in parentheses.
fn rule_body(body: &str) -> String {
    let b = body.trim().trim_end_matches(';').trim();
    if b.is_empty() {
        return "NOTHING".to_string();
    }
    if b.contains(';') && !b.starts_with('(') {
        return format!("({})", b);
    }
    b.to_string()
}

#[cfg(test)]
mod tests {
    use super::{add_exclude, create_rule};
    use crate::domain::{DbKind, ExcludeDef, RuleDef};
    use crate::sql::dialect_for;

    #[test]
    fn exclude_with_method_and_predicate() {
        let pg = dialect_for(DbKind::PostgreSql);
        let e = ExcludeDef { name: "no_overlap".into(), method: "GIST".into(), elements: "room WITH =, during WITH &&".into(), predicate: "NOT cancelled".into(), ..Default::default() };
        assert_eq!(add_exclude(pg, "booking", &e), "ALTER TABLE \"booking\" ADD CONSTRAINT \"no_overlap\" EXCLUDE USING gist (room WITH =, during WITH &&) WHERE (NOT cancelled)");
    }

    #[test]
    fn rule_disabled_with_comment_and_two_commands() {
        let pg = dialect_for(DbKind::PostgreSql);
        let r = RuleDef { name: "r".into(), event: "delete".into(), instead: true, body: "UPDATE t SET gone = true WHERE id = old.id; NOTIFY t;".into(), comment: "soft".into(), ..Default::default() };
        assert_eq!(
            create_rule(pg, "t", &r),
            [
                "CREATE RULE \"r\" AS ON DELETE TO \"t\" DO INSTEAD (UPDATE t SET gone = true WHERE id = old.id; NOTIFY t)",
                "ALTER TABLE \"t\" DISABLE RULE \"r\"",
                "COMMENT ON RULE \"r\" ON \"t\" IS 'soft'",
            ]
        );
    }

    #[test]
    fn rule_without_body_does_nothing_and_bad_event_makes_no_sql() {
        let pg = dialect_for(DbKind::PostgreSql);
        let r = RuleDef { name: "r".into(), event: "INSERT".into(), condition: "new.a < 0".into(), enabled: true, ..Default::default() };
        assert_eq!(create_rule(pg, "t", &r), ["CREATE RULE \"r\" AS ON INSERT TO \"t\" WHERE new.a < 0 DO ALSO NOTHING"]);
        assert!(create_rule(pg, "t", &RuleDef { event: "TRUNCATE".into(), ..r }).is_empty());
    }
}
