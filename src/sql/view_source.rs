//! An existing view's definition, split for the Design View editor: the header that
//! (re)defines it, kept out of sight, and the query the user edits.

use super::dialect::Dialect;
use super::sql_tokens::{tokens, Kind};

/// `ddl` (the server's CREATE VIEW text) split at the `AS` after `VIEW`: the header
/// with its leading CREATE turned into the dialect's way of changing a view (options
/// such as a column list, SCHEMABINDING or DEFINER stay in it), and the query.
/// None when the text has no such `AS`.
pub fn split_view(d: &dyn Dialect, ddl: &str) -> Option<(String, String)> {
    let toks = tokens(ddl);
    let view = toks.iter().position(|t| t.is("VIEW"))?;
    let as_tok = toks[view..].iter().find(|t| t.is("AS"))?;
    let as_end = offset(ddl, as_tok.text) + as_tok.text.len();
    Some((redefine(d, &ddl[..as_end]), ddl[as_end..].trim().to_string()))
}

/// What Run sends: the header and the edited query, without the `;` that would end it early.
pub fn view_statement(head: &str, query: &str) -> String {
    format!("{}\n{}", head.trim_end(), query.trim().trim_end_matches(';').trim_end())
}

/// Byte offset of `part`, a slice of `whole`.
fn offset(whole: &str, part: &str) -> usize {
    part.as_ptr() as usize - whole.as_ptr() as usize
}

/// The header with its first word CREATE replaced; a CREATE OR … header is kept.
fn redefine(d: &dyn Dialect, head: &str) -> String {
    let toks = tokens(head);
    let mut words = toks.iter().filter(|t| t.kind != Kind::LineComment && !t.text.starts_with("/*"));
    let Some(create) = words.next().filter(|t| t.is("CREATE")) else { return head.to_string() };
    if words.next().is_some_and(|t| t.is("OR")) {
        return head.to_string();
    }
    let at = offset(head, create.text);
    format!("{}{}{}", &head[..at], d.redefine_view(), &head[at + create.text.len()..])
}

#[cfg(test)]
mod tests {
    use super::{split_view, view_statement};
    use crate::domain::DbKind;
    use crate::sql::dialect_for;

    #[test]
    fn views_split_into_header_and_query() {
        let mssql = dialect_for(DbKind::SqlServer);
        let (head, query) = split_view(mssql, "-- totals\r\ncreate view [dbo].[v as x] WITH SCHEMABINDING AS\r\nSELECT 1 AS a").expect("split");
        assert_eq!((head.as_str(), query.as_str()), ("-- totals\r\nALTER view [dbo].[v as x] WITH SCHEMABINDING AS", "SELECT 1 AS a"));
        assert_eq!(view_statement(&head, "SELECT 2;\n"), "-- totals\r\nALTER view [dbo].[v as x] WITH SCHEMABINDING AS\nSELECT 2");
        let mysql = dialect_for(DbKind::MySql);
        let (head, query) = split_view(mysql, "CREATE ALGORITHM=UNDEFINED DEFINER=`root`@`%` VIEW `v` AS select 1 AS `a`;").expect("split");
        assert_eq!((head.as_str(), query.as_str()), ("CREATE OR REPLACE ALGORITHM=UNDEFINED DEFINER=`root`@`%` VIEW `v` AS", "select 1 AS `a`;"));
        let pg = dialect_for(DbKind::PostgreSql);
        assert_eq!(split_view(pg, "CREATE OR REPLACE VIEW v AS\n SELECT 1;").map(|s| s.0), Some("CREATE OR REPLACE VIEW v AS".to_string()));
        assert_eq!(split_view(pg, "SELECT 1"), None);
    }
}
