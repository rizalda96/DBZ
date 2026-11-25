//! An index's column list as rows for the index fields editor: read from the text the
//! designer keeps in `IndexDef::columns` and written back to it.

use super::dialect::Dialect;
use super::parts::{is_plain, pick, split_outside, split_top};
use crate::domain::IndexColumn;

/// Sort orders offered per index column; "" leaves it to the server.
pub const SORT_ORDERS: [&str; 3] = ["", "ASC", "DESC"];
/// NULLS orders offered per index column (PostgreSQL); "" leaves it to the server.
pub const NULLS_ORDERS: [&str; 3] = ["", "NULLS FIRST", "NULLS LAST"];

/// "a DESC, lower(b)" → one row per entry. An entry that can't be taken apart keeps its
/// whole text as the name, so nothing the user typed is lost.
pub fn parse_index_columns(text: &str) -> Vec<IndexColumn> {
    split_top(text).into_iter().map(|item| parse_item(&item).unwrap_or(IndexColumn { name: item, ..Default::default() })).collect()
}

/// The rows as the column-list text; empty names are left out.
pub fn index_columns_text(d: &dyn Dialect, cols: &[IndexColumn]) -> String {
    cols.iter().map(|c| item_text(d, c)).filter(|s| !s.is_empty()).collect::<Vec<_>>().join(", ")
}

/// How a table column is written in the list: plain names as they are (the list
/// quotes them when it builds SQL), anything else quoted.
pub fn index_column_ref(d: &dyn Dialect, column: &str) -> String {
    if is_plain(column) {
        column.to_string()
    } else {
        d.quote_ident(column)
    }
}

fn parse_item(item: &str) -> Option<IndexColumn> {
    let words = split_outside(item, char::is_whitespace);
    let (head, rest) = words.split_first()?;
    let mut col = IndexColumn::default();
    (col.name, col.sub_part) = name_and_prefix(head);
    let mut it = rest.iter().map(String::as_str);
    while let Some(word) = it.next() {
        let upper = word.to_ascii_uppercase();
        match upper.as_str() {
            "ASC" | "DESC" => col.order = upper,
            "NULLS" => col.nulls = nulls(it.next()?)?,
            "COLLATE" => (col.collation_schema, col.collation) = qualified(it.next()?)?,
            "WITH" => col.operator = it.next().filter(|op| is_operator(op))?.to_string(),
            _ if col.opclass.is_empty() => (col.opclass_schema, col.opclass) = qualified(word)?,
            _ => return None,
        }
    }
    Some(col)
}

/// A PostgreSQL operator name such as `=`, `&&` or `<->`.
fn is_operator(word: &str) -> bool {
    (1..=63).contains(&word.len()) && word.chars().all(|c| "+-*/<>=~!@#%^&|`?".contains(c))
}

fn nulls(word: &str) -> Option<String> {
    let w = word.to_ascii_uppercase();
    matches!(w.as_str(), "FIRST" | "LAST").then(|| format!("NULLS {}", w))
}

/// `title(10)` → ("title", "10"); a name that is a quoted plain word loses its quotes,
/// since the list quotes plain names anyway.
fn name_and_prefix(head: &str) -> (String, String) {
    let prefix = head.strip_suffix(')').and_then(|h| h.rsplit_once('(')).filter(|(base, n)| !n.is_empty() && n.chars().all(|c| c.is_ascii_digit()) && ident(base).is_some());
    match prefix {
        Some((base, n)) => (plain_name(base), n.to_string()),
        None => (plain_name(head), String::new()),
    }
}

fn plain_name(word: &str) -> String {
    if is_plain(word) {
        return word.to_string();
    }
    unquote(word).filter(|w| is_plain(w)).unwrap_or_else(|| word.to_string())
}

/// `"pg_catalog"."text_ops"` or `text_ops` → (schema, name), unquoted.
fn qualified(word: &str) -> Option<(String, String)> {
    let parts = split_outside(word, |c| c == '.');
    match parts.as_slice() {
        [name] => Some((String::new(), ident(name)?)),
        [schema, name] => Some((ident(schema)?, ident(name)?)),
        _ => None,
    }
}

/// The identifier a word spells: quoted ones as written inside the quotes, bare ones
/// folded to lower case the way PostgreSQL folds them. None for anything else.
fn ident(word: &str) -> Option<String> {
    unquote(word).or_else(|| is_plain(word).then(|| word.to_lowercase()))
}

/// The inside of a `"…"` or `` `…` `` word, with doubled quotes undone.
fn unquote(word: &str) -> Option<String> {
    ['"', '`'].into_iter().find_map(|q| {
        let inner = word.strip_prefix(q)?.strip_suffix(q).filter(|i| !i.is_empty())?;
        Some(inner.replace(&format!("{q}{q}"), &q.to_string()))
    })
}

fn item_text(d: &dyn Dialect, c: &IndexColumn) -> String {
    let name = c.name.trim();
    if name.is_empty() {
        return String::new();
    }
    let sub_part = Some(c.sub_part.trim()).filter(|n| !n.is_empty() && n.chars().all(|ch| ch.is_ascii_digit()));
    let collation = Some(c.collation.trim()).filter(|s| !s.is_empty()).map(|s| format!("COLLATE {}", qualify(d, &c.collation_schema, s)));
    let opclass = Some(c.opclass.trim()).filter(|s| !s.is_empty()).map(|s| qualify(d, &c.opclass_schema, s));
    let operator = Some(c.operator.trim()).filter(|op| is_operator(op)).map(|op| format!("WITH {}", op));
    // The list quotes a plain name only when ASC / DESC / NULLS follow it; with
    // anything else after it the name is quoted here.
    let decorated = sub_part.is_some() || collation.is_some() || opclass.is_some() || operator.is_some();
    let mut head = if decorated && is_plain(name) { d.quote_ident(name) } else { name.to_string() };
    if let Some(n) = sub_part {
        head.push_str(&format!("({})", n));
    }
    let order = pick(&c.order, &SORT_ORDERS[1..]).map(str::to_string);
    let nulls = pick(&c.nulls, &NULLS_ORDERS[1..]).map(str::to_string);
    std::iter::once(head).chain(collation).chain(opclass).chain(order).chain(nulls).chain(operator).collect::<Vec<_>>().join(" ")
}

fn qualify(d: &dyn Dialect, schema: &str, name: &str) -> String {
    match schema.trim() {
        "" => d.quote_ident(name),
        s => format!("{}.{}", d.quote_ident(s), d.quote_ident(name)),
    }
}

#[cfg(test)]
mod tests {
    use super::{index_column_ref, index_columns_text, parse_index_columns};
    use crate::domain::{DbKind, IndexColumn, IndexDef};
    use crate::sql::dialect_for;

    fn col(name: &str) -> IndexColumn {
        IndexColumn { name: name.into(), ..Default::default() }
    }

    #[test]
    fn reads_what_postgres_lists() {
        let cols = parse_index_columns("id_site, name COLLATE \"C\" text_pattern_ops DESC, lower(email)");
        assert_eq!(cols[0], col("id_site"));
        let named = IndexColumn { collation: "C".into(), opclass: "text_pattern_ops".into(), order: "DESC".into(), ..col("name") };
        assert_eq!(cols[1], named);
        assert_eq!(cols[2], col("lower(email)"));
    }

    #[test]
    fn writes_collation_opclass_and_order() {
        let pg = dialect_for(DbKind::PostgreSql);
        let c = IndexColumn { opclass_schema: "pg_catalog".into(), opclass: "uuid_ops".into(), order: "ASC".into(), nulls: "NULLS LAST".into(), ..col("id_site") };
        let text = index_columns_text(pg, &[c.clone(), col("lower(email)")]);
        assert_eq!(text, "\"id_site\" \"pg_catalog\".\"uuid_ops\" ASC NULLS LAST, lower(email)");
        assert_eq!(parse_index_columns(&text), [c, col("lower(email)")]);
    }

    #[test]
    fn written_text_builds_the_index() {
        let pg = dialect_for(DbKind::PostgreSql);
        let c = IndexColumn { opclass_schema: "pg_catalog".into(), opclass: "uuid_ops".into(), order: "DESC".into(), ..col("id_site") };
        let columns = index_columns_text(pg, &[c, IndexColumn { nulls: "NULLS LAST".into(), ..col("Code") }]);
        let ix = IndexDef { name: "ix".into(), columns, ..Default::default() };
        assert_eq!(pg.create_index("t", &ix), ["CREATE INDEX \"ix\" ON \"t\" (\"id_site\" \"pg_catalog\".\"uuid_ops\" DESC, \"Code\" NULLS LAST)"]);
    }

    #[test]
    fn exclude_elements_keep_their_operator() {
        let pg = dialect_for(DbKind::PostgreSql);
        let cols = parse_index_columns("room WITH =, during gist_ops WITH &&");
        assert_eq!(cols[0], IndexColumn { operator: "=".into(), ..col("room") });
        assert_eq!(cols[1], IndexColumn { opclass: "gist_ops".into(), operator: "&&".into(), ..col("during") });
        assert_eq!(index_columns_text(pg, &cols), "\"room\" WITH =, \"during\" \"gist_ops\" WITH &&");
        let bad = IndexColumn { operator: "=; DROP".into(), ..col("a") };
        assert_eq!(index_columns_text(pg, &[bad]), "a");
    }

    #[test]
    fn mysql_prefix_round_trips() {
        let my = dialect_for(DbKind::MySql);
        let cols = parse_index_columns("title(10) DESC, `code`");
        assert_eq!(cols, [IndexColumn { sub_part: "10".into(), order: "DESC".into(), ..col("title") }, col("code")]);
        assert_eq!(index_columns_text(my, &cols), "`title`(10) DESC, code");
    }

    #[test]
    fn entries_it_cannot_read_stay_whole_and_bad_values_are_dropped() {
        assert_eq!(parse_index_columns("a || b"), [col("a || b")]);
        let pg = dialect_for(DbKind::PostgreSql);
        let c = IndexColumn { sub_part: "1); DROP".into(), order: "DESC; DROP TABLE t".into(), ..col("a") };
        assert_eq!(index_columns_text(pg, &[c, col("  ")]), "a");
    }

    #[test]
    fn odd_column_names_are_quoted_when_picked() {
        let pg = dialect_for(DbKind::PostgreSql);
        assert_eq!(index_column_ref(pg, "id_site"), "id_site");
        assert_eq!(index_column_ref(pg, "My Col"), "\"My Col\"");
        assert_eq!(parse_index_columns("\"My Col\" DESC")[0], IndexColumn { order: "DESC".into(), ..col("\"My Col\"") });
    }
}
