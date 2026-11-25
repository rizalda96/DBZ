//! SQL for the designer's indexes, keys, checks, triggers and options. Statements every
//! server shares are built here; what differs comes from the `Dialect`.

use super::dialect::Dialect;
use super::exclude_rule::{add_exclude, create_rule, drop_rule};
use crate::domain::{CheckDef, ForeignKeyDef, Part, TableParts, UniqueDef};

/// Referential actions offered for ON DELETE / ON UPDATE.
pub const FK_ACTIONS: [&str; 5] = ["NO ACTION", "RESTRICT", "CASCADE", "SET NULL", "SET DEFAULT"];

/// Words allowed after a plain column name in an index column list.
const ORDER_WORDS: [&str; 5] = ["ASC", "DESC", "NULLS", "FIRST", "LAST"];

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ConstraintKind {
    ForeignKey,
    Unique,
    Check,
    Exclude,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum OptionKind {
    Text,
    /// Stored as "true" / "false".
    Bool,
}

/// One field of the designer's Options tab.
pub struct OptionSpec {
    pub key: &'static str,
    pub label: &'static str,
    pub kind: OptionKind,
}

/// One storage parameter of an index (PostgreSQL `WITH (key = value)`).
pub struct IndexParamSpec {
    pub key: &'static str,
    pub label: &'static str,
    /// The values offered; empty = a whole number typed in.
    pub values: &'static [&'static str],
    /// Index methods that take it.
    pub methods: &'static [&'static str],
}

impl IndexParamSpec {
    /// Whether an index using `method` (the default btree when empty) takes it.
    pub fn applies(&self, method: &str) -> bool {
        let method = match method.trim() {
            "" => "btree",
            m => m,
        };
        self.methods.iter().any(|m| m.eq_ignore_ascii_case(method))
    }

    /// `value` as it may appear in SQL, or None when it isn't one this parameter takes.
    pub(super) fn valid<'a>(&self, value: &'a str) -> Option<&'a str> {
        let v = value.trim();
        let ok = if self.values.is_empty() { !v.is_empty() && v.chars().all(|c| c.is_ascii_digit()) } else { self.values.iter().any(|x| x.eq_ignore_ascii_case(v)) };
        ok.then_some(v)
    }
}

/// What the designer's tabs offer on a server.
pub struct DesignerCaps {
    pub index_methods: &'static [&'static str],
    pub trigger_timings: &'static [&'static str],
    /// MySQL triggers fire on exactly one event, always FOR EACH ROW.
    pub trigger_one_event: bool,
    /// TRUNCATE and UPDATE OF events, statement-level and disabled triggers (PostgreSQL).
    pub trigger_extras: bool,
    pub check_no_inherit: bool,
    /// Comments on constraints and triggers (PostgreSQL); MySQL keeps them on indexes only.
    pub constraint_comments: bool,
    /// Header of the trigger column that holds the function call or statement.
    pub trigger_body: &'static str,
    /// Index columns take a prefix length (MySQL).
    pub index_sub_part: bool,
    /// Index columns take a collation, an operator class and a NULLS order (PostgreSQL).
    pub index_collation_opclass: bool,
    /// CREATE INDEX CONCURRENTLY (PostgreSQL).
    pub index_concurrently: bool,
    /// Constraint options (PostgreSQL): foreign keys take MATCH FULL, uniques a
    /// tablespace and fill factor, and both DEFERRABLE.
    pub constraint_options: bool,
    /// Exclusion constraints and rewrite rules (PostgreSQL); their tabs show only then.
    pub excludes_and_rules: bool,
    /// Storage parameters an index can take (PostgreSQL).
    pub index_params: &'static [IndexParamSpec],
    /// Index tablespace and partial-index WHERE (PostgreSQL).
    pub index_placement: bool,
    pub options: &'static [OptionSpec],
}

/// The designer's parts before and after editing.
pub struct PartsChange<'a> {
    /// The table's name on the server now; drops use it.
    pub table: &'a str,
    /// Its name once the column changes (and a rename) ran; creates use it.
    pub new_table: &'a str,
    pub old: &'a TableParts,
    pub new: &'a TableParts,
}

/// Drops run before the column changes, creates after them.
#[derive(Default, Debug)]
pub struct PartsSql {
    pub drops: Vec<String>,
    pub creates: Vec<String>,
}

pub fn parts_sql(d: &dyn Dialect, c: &PartsChange) -> PartsSql {
    let (t, nt) = (c.table, c.new_table);
    let (drop_tr, add_tr) = changes(&c.old.triggers, &c.new.triggers);
    let (drop_fk, add_fk) = changes(&c.old.foreign_keys, &c.new.foreign_keys);
    let (drop_ck, add_ck) = changes(&c.old.checks, &c.new.checks);
    let (drop_ex, add_ex) = changes(&c.old.excludes, &c.new.excludes);
    let (drop_ru, add_ru) = changes(&c.old.rules, &c.new.rules);
    let (drop_uq, add_uq) = changes(&c.old.uniques, &c.new.uniques);
    let (drop_ix, add_ix) = changes(&c.old.indexes, &c.new.indexes);
    let mut out = PartsSql::default();
    out.drops.extend(drop_tr.iter().map(|p| d.drop_trigger(t, &p.name)));
    out.drops.extend(drop_ru.iter().map(|p| drop_rule(d, t, &p.name)));
    out.drops.extend(drop_fk.iter().map(|p| d.drop_constraint(t, ConstraintKind::ForeignKey, &p.name)));
    out.drops.extend(drop_ck.iter().map(|p| d.drop_constraint(t, ConstraintKind::Check, &p.name)));
    out.drops.extend(drop_ex.iter().map(|p| d.drop_constraint(t, ConstraintKind::Exclude, &p.name)));
    out.drops.extend(drop_uq.iter().map(|p| d.drop_constraint(t, ConstraintKind::Unique, &p.name)));
    out.drops.extend(drop_ix.iter().map(|p| d.drop_index(t, &p.name)));
    out.creates.extend(add_ix.iter().flat_map(|p| d.create_index(nt, p)));
    out.creates.extend(add_uq.iter().flat_map(|p| commented(d, nt, (&p.name, &p.comment), add_unique(d, nt, p))));
    out.creates.extend(add_ck.iter().flat_map(|p| commented(d, nt, (&p.name, &p.comment), add_check(d, nt, p))));
    out.creates.extend(add_ex.iter().flat_map(|p| commented(d, nt, (&p.name, &p.comment), add_exclude(d, nt, p))));
    out.creates.extend(add_fk.iter().flat_map(|p| commented(d, nt, (&p.name, &p.comment), add_foreign_key(d, nt, p))));
    out.creates.extend(add_ru.iter().flat_map(|p| create_rule(d, nt, p)));
    out.creates.extend(add_tr.iter().flat_map(|p| d.create_trigger(nt, p)));
    out.creates.extend(d.table_options(nt, &c.old.options, &c.new.options));
    out
}

/// Old parts to drop and new parts to create. A changed part is both: it is dropped
/// and created again, which is how servers change most constraints anyway. Parts
/// without a name are left out.
fn changes<'a, T: Part>(old: &'a [T], new: &'a [T]) -> (Vec<&'a T>, Vec<&'a T>) {
    let same = |a: &T, b: &T| a.detached() == b.detached();
    let now = |o: &T| new.iter().find(|n| n.orig_name() == Some(o.name()));
    let was = |n: &T| n.orig_name().and_then(|on| old.iter().find(|o| o.name() == on));
    let drops = old.iter().filter(|&o| now(o).is_none_or(|n| !same(o, n))).collect();
    let creates = new.iter().filter(|&n| !n.name().trim().is_empty() && was(n).is_none_or(|o| !same(o, n))).collect();
    (drops, creates)
}

/// The statement, plus a comment on the constraint where the server keeps those.
fn commented(d: &dyn Dialect, table: &str, (name, comment): (&str, &str), sql: String) -> Vec<String> {
    let note = (!comment.is_empty()).then(|| d.constraint_comment(table, name, comment)).flatten();
    std::iter::once(sql).chain(note).collect()
}

fn add_unique(d: &dyn Dialect, table: &str, u: &UniqueDef) -> String {
    let mut s = format!("ALTER TABLE {} ADD CONSTRAINT {} UNIQUE ({})", d.quote_ident(table), d.quote_ident(&u.name), column_list(d, &u.columns));
    if !d.designer().constraint_options {
        return s;
    }
    // A fill factor PostgreSQL wouldn't take is left out rather than sent.
    if let Some(ff) = u.fill_factor.trim().parse::<u8>().ok().filter(|n| (10..=100).contains(n)) {
        s.push_str(&format!(" WITH (fillfactor = {})", ff));
    }
    if !u.tablespace.trim().is_empty() {
        s.push_str(&format!(" USING INDEX TABLESPACE {}", d.quote_ident(u.tablespace.trim())));
    }
    s.push_str(deferrable(u.deferrable, u.deferred));
    s
}

/// The DEFERRABLE clause of a constraint; empty when it isn't deferrable.
pub(super) fn deferrable(deferrable: bool, deferred: bool) -> &'static str {
    match (deferrable, deferred) {
        (false, _) => "",
        (true, false) => " DEFERRABLE INITIALLY IMMEDIATE",
        (true, true) => " DEFERRABLE INITIALLY DEFERRED",
    }
}

fn add_check(d: &dyn Dialect, table: &str, c: &CheckDef) -> String {
    let no_inherit = if c.no_inherit && d.designer().check_no_inherit { " NO INHERIT" } else { "" };
    format!("ALTER TABLE {} ADD CONSTRAINT {} CHECK ({}){}", d.quote_ident(table), d.quote_ident(&c.name), c.expr.trim(), no_inherit)
}

fn add_foreign_key(d: &dyn Dialect, table: &str, f: &ForeignKeyDef) -> String {
    let target = match f.ref_schema.trim() {
        "" => d.quote_ident(f.ref_table.trim()),
        schema => format!("{}.{}", d.quote_ident(schema), d.quote_ident(f.ref_table.trim())),
    };
    let mut s = format!(
        "ALTER TABLE {} ADD CONSTRAINT {} FOREIGN KEY ({}) REFERENCES {} ({})",
        d.quote_ident(table),
        d.quote_ident(&f.name),
        column_list(d, &f.columns),
        target,
        column_list(d, &f.ref_columns)
    );
    let extras = d.designer().constraint_options;
    if extras && f.match_full {
        s.push_str(" MATCH FULL");
    }
    for (event, action) in [("DELETE", &f.on_delete), ("UPDATE", &f.on_update)] {
        // NO ACTION is what every server does without the clause.
        if let Some(a) = pick(action, &FK_ACTIONS).filter(|a| *a != "NO ACTION") {
            s.push_str(&format!(" ON {} {}", event, a));
        }
    }
    if extras {
        s.push_str(deferrable(f.deferrable, f.deferred));
    }
    s
}

/// `value` as spelled in `allowed` (any case), or None when it is not one of them.
pub(super) fn pick(value: &str, allowed: &[&'static str]) -> Option<&'static str> {
    let v = value.trim();
    allowed.iter().copied().find(|a| a.eq_ignore_ascii_case(v))
}

/// A bare word for a table option (engine, charset, …); None for anything else, so
/// option values can't carry other SQL.
pub(super) fn plain_word(v: &str) -> Option<&str> {
    let v = v.trim();
    (!v.is_empty() && v.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')).then_some(v)
}

/// "a, b DESC" with plain names quoted. Anything else (expressions, names already
/// quoted) is SQL the user typed in the designer and passes as written.
pub(super) fn column_list(d: &dyn Dialect, text: &str) -> String {
    split_top(text).iter().map(|item| column_item(d, item)).filter(|s| !s.is_empty()).collect::<Vec<_>>().join(", ")
}

fn column_item(d: &dyn Dialect, item: &str) -> String {
    let mut words = item.split_whitespace();
    let Some(head) = words.next() else { return String::new() };
    let rest: Vec<String> = words.map(str::to_uppercase).collect();
    if !is_plain(head) || !rest.iter().all(|w| ORDER_WORDS.contains(&w.as_str())) {
        return item.trim().to_string();
    }
    std::iter::once(d.quote_ident(head)).chain(rest).collect::<Vec<_>>().join(" ")
}

/// A bare identifier: letters, digits and `_`, not starting with a digit.
pub(super) fn is_plain(word: &str) -> bool {
    !word.is_empty() && word.chars().all(|c| c.is_alphanumeric() || c == '_') && !word.starts_with(|c: char| c.is_ascii_digit())
}

/// Splits on commas outside parentheses and quotes.
pub(super) fn split_top(text: &str) -> Vec<String> {
    split_outside(text, |c| c == ',')
}

/// Splits on `sep` characters outside parentheses and quotes; pieces come trimmed and
/// empty ones are left out.
pub(super) fn split_outside(text: &str, sep: impl Fn(char) -> bool) -> Vec<String> {
    let mut out = vec![];
    let (mut cur, mut depth, mut quote) = (String::new(), 0i32, None::<char>);
    for ch in text.chars() {
        match (quote, ch) {
            (Some(q), _) if ch == q => quote = None,
            (Some(_), _) => {}
            (None, '\'' | '"' | '`') => quote = Some(ch),
            (None, '(') => depth += 1,
            (None, ')') => depth -= 1,
            (None, _) if depth == 0 && sep(ch) => {
                out.push(std::mem::take(&mut cur));
                continue;
            }
            (None, _) => {}
        }
        cur.push(ch);
    }
    out.push(cur);
    out.into_iter().map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect()
}

#[cfg(test)]
mod tests {
    use super::{column_list, parts_sql, PartsChange};
    use crate::domain::{DbKind, ForeignKeyDef, IndexDef, TableParts, UniqueDef};
    use crate::sql::dialect_for;

    fn unique(name: &str, cols: &str) -> UniqueDef {
        UniqueDef { name: name.into(), columns: cols.into(), ..Default::default() }
    }

    #[test]
    fn quotes_plain_columns_and_keeps_expressions() {
        let pg = dialect_for(DbKind::PostgreSql);
        assert_eq!(column_list(pg, "id_site desc, lower(name), \"Odd\""), "\"id_site\" DESC, lower(name), \"Odd\"");
    }

    #[test]
    fn changed_part_is_dropped_and_created_again() {
        let pg = dialect_for(DbKind::PostgreSql);
        let old = TableParts { uniques: vec![unique("u1", "a")], ..Default::default() }.loaded();
        let mut new = old.clone();
        new.uniques[0].columns = "a, b".into();
        let sql = parts_sql(pg, &PartsChange { table: "t", new_table: "t", old: &old, new: &new });
        assert_eq!(sql.drops, ["ALTER TABLE \"t\" DROP CONSTRAINT \"u1\""]);
        assert_eq!(sql.creates, ["ALTER TABLE \"t\" ADD CONSTRAINT \"u1\" UNIQUE (\"a\", \"b\")"]);
    }

    #[test]
    fn unchanged_parts_produce_nothing() {
        let my = dialect_for(DbKind::MySql);
        let ix = IndexDef { name: "ix".into(), columns: "a".into(), ..Default::default() };
        let old = TableParts { indexes: vec![ix], ..Default::default() }.loaded();
        let sql = parts_sql(my, &PartsChange { table: "t", new_table: "t", old: &old, new: &old.clone() });
        assert!(sql.drops.is_empty() && sql.creates.is_empty());
    }

    #[test]
    fn concurrently_builds_new_indexes_but_alone_changes_nothing() {
        let pg = dialect_for(DbKind::PostgreSql);
        let ix = IndexDef { name: "ix".into(), columns: "a".into(), ..Default::default() };
        let old = TableParts { indexes: vec![ix], ..Default::default() }.loaded();
        let mut new = old.clone();
        new.indexes[0].concurrently = true;
        let sql = parts_sql(pg, &PartsChange { table: "t", new_table: "t", old: &old, new: &new });
        assert!(sql.drops.is_empty() && sql.creates.is_empty());
        new.indexes[0].columns = "a, b".into();
        let sql = parts_sql(pg, &PartsChange { table: "t", new_table: "t", old: &old, new: &new });
        assert_eq!(sql.creates, ["CREATE INDEX CONCURRENTLY \"ix\" ON \"t\" (\"a\", \"b\")"]);
    }

    #[test]
    fn index_properties_follow_the_method() {
        let pg = dialect_for(DbKind::PostgreSql);
        let storage = [("fillfactor", "70"), ("buffering", "on"), ("fastupdate", "x")].into_iter().map(|(k, v)| (k.to_string(), v.to_string())).collect();
        let ix = IndexDef { name: "ix".into(), columns: "a".into(), storage, tablespace: "fast".into(), predicate: "a > 0".into(), ..Default::default() };
        let new = TableParts { indexes: vec![ix], ..Default::default() };
        let sql = parts_sql(pg, &PartsChange { table: "t", new_table: "t", old: &TableParts::default(), new: &new });
        assert_eq!(sql.creates, ["CREATE INDEX \"ix\" ON \"t\" (\"a\") WITH (fillfactor = 70) TABLESPACE \"fast\" WHERE a > 0"]);
    }

    #[test]
    fn empty_index_parameters_are_no_change() {
        let pg = dialect_for(DbKind::PostgreSql);
        let ix = IndexDef { name: "ix".into(), columns: "a".into(), ..Default::default() };
        let old = TableParts { indexes: vec![ix], ..Default::default() }.loaded();
        let mut new = old.clone();
        new.indexes[0].storage.insert("fillfactor".into(), String::new());
        let sql = parts_sql(pg, &PartsChange { table: "t", new_table: "t", old: &old, new: &new });
        assert!(sql.drops.is_empty() && sql.creates.is_empty());
    }

    #[test]
    fn foreign_key_match_and_deferrable_only_where_the_server_has_them() {
        let fk = ForeignKeyDef {
            name: "fk_site".into(),
            columns: "id_site".into(),
            ref_schema: "public".into(),
            ref_table: "master_site".into(),
            ref_columns: "id".into(),
            on_delete: "CASCADE".into(),
            on_update: "NO ACTION".into(),
            match_full: true,
            deferrable: true,
            deferred: true,
            ..Default::default()
        };
        let pg = super::add_foreign_key(dialect_for(DbKind::PostgreSql), "t", &fk);
        assert_eq!(pg, "ALTER TABLE \"t\" ADD CONSTRAINT \"fk_site\" FOREIGN KEY (\"id_site\") REFERENCES \"public\".\"master_site\" (\"id\") MATCH FULL ON DELETE CASCADE DEFERRABLE INITIALLY DEFERRED");
        let my = super::add_foreign_key(dialect_for(DbKind::MySql), "t", &fk);
        assert_eq!(my, "ALTER TABLE `t` ADD CONSTRAINT `fk_site` FOREIGN KEY (`id_site`) REFERENCES `public`.`master_site` (`id`) ON DELETE CASCADE");
    }

    #[test]
    fn unique_options_only_where_the_server_has_them() {
        let u = UniqueDef { tablespace: "fast".into(), fill_factor: "70".into(), deferrable: true, ..unique("uq", "id_transporter, id_site") };
        let pg = super::add_unique(dialect_for(DbKind::PostgreSql), "t", &u);
        assert_eq!(pg, "ALTER TABLE \"t\" ADD CONSTRAINT \"uq\" UNIQUE (\"id_transporter\", \"id_site\") WITH (fillfactor = 70) USING INDEX TABLESPACE \"fast\" DEFERRABLE INITIALLY IMMEDIATE");
        let odd = UniqueDef { fill_factor: "5); DROP".into(), deferrable: false, ..u.clone() };
        assert_eq!(super::add_unique(dialect_for(DbKind::PostgreSql), "t", &odd), "ALTER TABLE \"t\" ADD CONSTRAINT \"uq\" UNIQUE (\"id_transporter\", \"id_site\") USING INDEX TABLESPACE \"fast\"");
        assert_eq!(super::add_unique(dialect_for(DbKind::MySql), "t", &u), "ALTER TABLE `t` ADD CONSTRAINT `uq` UNIQUE (`id_transporter`, `id_site`)");
    }

    #[test]
    fn new_mysql_index_and_dropped_one() {
        let my = dialect_for(DbKind::MySql);
        let old = TableParts { indexes: vec![IndexDef { name: "old_ix".into(), columns: "a".into(), ..Default::default() }], ..Default::default() }.loaded();
        let new = TableParts { indexes: vec![IndexDef { name: "ix_b".into(), columns: "b".into(), method: "btree".into(), ..Default::default() }], ..Default::default() };
        let sql = parts_sql(my, &PartsChange { table: "t", new_table: "t", old: &old, new: &new });
        assert_eq!(sql.drops, ["DROP INDEX `old_ix` ON `t`"]);
        assert_eq!(sql.creates, ["CREATE INDEX `ix_b` ON `t` (`b`) USING BTREE"]);
    }
}
