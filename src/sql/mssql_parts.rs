//! SQL Server statements for the designer's indexes and triggers.

use super::dialect::Dialect;
use super::mssql::{split_name, MssqlDialect};
use super::parts::{column_list, pick, DesignerCaps};
use crate::domain::{IndexDef, TriggerDef};

pub(super) static CAPS: DesignerCaps = DesignerCaps {
    index_methods: &["NONCLUSTERED", "CLUSTERED"],
    trigger_timings: &["AFTER", "INSTEAD OF"],
    trigger_one_event: false,
    trigger_extras: false,
    check_no_inherit: false,
    constraint_comments: false,
    trigger_body: "Statement",
    trigger_enable: true,
    trigger_comments: true,
    trigger_options: true,
    index_sub_part: false,
    index_collation_opclass: false,
    index_concurrently: false,
    constraint_options: false,
    excludes_and_rules: false,
    index_params: &[],
    index_placement: false,
    options: &[],
};

/// `CREATE [UNIQUE] [CLUSTERED | NONCLUSTERED] INDEX`; the method column picks the kind.
pub(super) fn create_index(d: &MssqlDialect, table: &str, i: &IndexDef) -> Vec<String> {
    let unique = if i.unique { "UNIQUE " } else { "" };
    let kind = pick(&i.method, CAPS.index_methods).map(|m| format!("{} ", m)).unwrap_or_default();
    vec![format!("CREATE {}{}INDEX {} ON {} ({})", unique, kind, d.quote_ident(split_name(&i.name).1), d.quote_ident(table), column_list(d, &i.columns))]
}

/// What a trigger can run as besides its caller's default; "USER" names a user.
pub const EXECUTE_AS: [&str; 4] = ["CALLER", "OWNER", "SELF", "USER"];

/// The trigger lives in its table's schema; a disabled one is disabled right after,
/// and a comment becomes its MS_Description. Empty when it has no event or no body.
pub(super) fn create_trigger(d: &MssqlDialect, table: &str, t: &TriggerDef) -> Vec<String> {
    let events: Vec<&str> = [(t.insert, "INSERT"), (t.update, "UPDATE"), (t.delete, "DELETE")].into_iter().filter(|(on, _)| *on).map(|(_, e)| e).collect();
    let body = t.body.trim();
    if events.is_empty() || body.is_empty() {
        return vec![];
    }
    let name = trigger_name(d, table, &t.name);
    let mut head = format!("CREATE TRIGGER {} ON {}", name, d.quote_ident(table));
    let options = trigger_options(d, t);
    if !options.is_empty() {
        head.push_str(&format!(" WITH {}", options.join(", ")));
    }
    let timing = pick(&t.fires, CAPS.trigger_timings).unwrap_or("AFTER");
    head.push_str(&format!(" {} {}", timing, events.join(", ")));
    if t.with_append {
        head.push_str(" WITH APPEND");
    }
    if t.not_for_replication {
        head.push_str(" NOT FOR REPLICATION");
    }
    let create = if t.external { format!("{} AS EXTERNAL NAME {}", head, body) } else { format!("{} AS\n{}", head, body) };
    let mut out = vec![create];
    if !t.enabled {
        out.push(format!("DISABLE TRIGGER {} ON {}", name, d.quote_ident(table)));
    }
    if !t.comment.trim().is_empty() {
        out.push(trigger_comment(d, table, t));
    }
    out
}

/// ENCRYPTION (not for CLR triggers, whose code isn't text) and EXECUTE AS.
fn trigger_options(d: &MssqlDialect, t: &TriggerDef) -> Vec<String> {
    let mut options = vec![];
    if t.encrypted && !t.external {
        options.push("ENCRYPTION".to_string());
    }
    let user = t.execute_as_user.trim();
    match pick(&t.execute_as, &EXECUTE_AS) {
        Some("USER") if !user.is_empty() => options.push(format!("EXECUTE AS '{}'", d.escape_str(user))),
        Some(who) if who != "USER" => options.push(format!("EXECUTE AS {}", who)),
        _ => {}
    }
    options
}

fn trigger_comment(d: &MssqlDialect, table: &str, t: &TriggerDef) -> String {
    let (schema, table) = split_name(table);
    format!(
        "EXEC sys.sp_addextendedproperty @name = N'MS_Description', @value = {}, @level0type = N'SCHEMA', @level0name = {}, \
         @level1type = N'TABLE', @level1name = {}, @level2type = N'TRIGGER', @level2name = {}",
        d.str_literal(t.comment.trim()),
        d.str_literal(schema),
        d.str_literal(table),
        d.str_literal(split_name(&t.name).1)
    )
}

pub(super) fn drop_trigger(d: &MssqlDialect, table: &str, name: &str) -> String {
    format!("DROP TRIGGER {}", trigger_name(d, table, name))
}

fn trigger_name(d: &MssqlDialect, table: &str, name: &str) -> String {
    format!("{}.{}", d.quote_ident(split_name(table).0), d.quote_ident(split_name(name).1))
}

#[cfg(test)]
mod tests {
    use crate::domain::{IndexDef, TriggerDef};
    use crate::sql::{Dialect, MssqlDialect};

    #[test]
    fn indexes_take_their_kind() {
        let i = IndexDef { name: "ix_site".into(), columns: "id_site DESC, made".into(), method: "clustered".into(), unique: true, ..Default::default() };
        assert_eq!(MssqlDialect.create_index("sales.orders", &i), ["CREATE UNIQUE CLUSTERED INDEX [ix_site] ON [sales].[orders] ([id_site] DESC, [made])"]);
        assert_eq!(MssqlDialect.drop_index("sales.orders", "ix_site"), "DROP INDEX [ix_site] ON [sales].[orders]");
    }

    #[test]
    fn triggers_fire_on_several_events() {
        let t = TriggerDef { name: "trg_audit".into(), fires: "AFTER".into(), insert: true, delete: true, enabled: true, body: "INSERT INTO audit SELECT * FROM inserted;".into(), ..Default::default() };
        assert_eq!(MssqlDialect.create_trigger("orders", &t), ["CREATE TRIGGER [dbo].[trg_audit] ON [orders] AFTER INSERT, DELETE AS\nINSERT INTO audit SELECT * FROM inserted;"]);
        assert!(MssqlDialect.create_trigger("orders", &TriggerDef { body: String::new(), ..t }).is_empty());
        assert_eq!(MssqlDialect.drop_trigger("sales.orders", "trg_audit"), "DROP TRIGGER [sales].[trg_audit]");
    }

    #[test]
    fn trigger_options_disabled_and_commented() {
        let t = TriggerDef {
            name: "trg_upd".into(),
            fires: "INSTEAD OF".into(),
            update: true,
            execute_as: "user".into(),
            execute_as_user: "o'brien".into(),
            encrypted: true,
            not_for_replication: true,
            comment: "Keeps it".into(),
            body: "SELECT 1".into(),
            ..Default::default()
        };
        assert_eq!(
            MssqlDialect.create_trigger("sales.orders", &t),
            [
                "CREATE TRIGGER [sales].[trg_upd] ON [sales].[orders] WITH ENCRYPTION, EXECUTE AS 'o''brien' INSTEAD OF UPDATE NOT FOR REPLICATION AS\nSELECT 1",
                "DISABLE TRIGGER [sales].[trg_upd] ON [sales].[orders]",
                "EXEC sys.sp_addextendedproperty @name = N'MS_Description', @value = N'Keeps it', @level0type = N'SCHEMA', @level0name = N'sales', \
                 @level1type = N'TABLE', @level1name = N'orders', @level2type = N'TRIGGER', @level2name = N'trg_upd'",
            ]
        );
        let clr = TriggerDef { external: true, execute_as: "OWNER".into(), with_append: true, fires: "AFTER".into(), enabled: true, comment: String::new(), body: "[Audit].[Triggers].[OnUpdate]".into(), ..t };
        assert_eq!(MssqlDialect.create_trigger("orders", &clr), ["CREATE TRIGGER [dbo].[trg_upd] ON [orders] WITH EXECUTE AS OWNER AFTER UPDATE WITH APPEND NOT FOR REPLICATION AS EXTERNAL NAME [Audit].[Triggers].[OnUpdate]"]);
    }
}
