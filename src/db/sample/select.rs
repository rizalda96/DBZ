//! SELECT: one table, WHERE, ORDER BY, LIMIT/OFFSET and COUNT(*).

use std::cmp::Ordering;

use super::cond::{cmp_cells, parse_where, row_matches, Where};
use super::parser::{Parser, Res};
use super::server::{SampleServer, SampleTable};
use super::tokens::Tok;
use crate::db::DbError;
use crate::domain::{Cell, ResultSet, Row};

enum Item {
    Star,
    Col(String, String),
    Count(String),
    Lit(Cell, String),
}

/// Where an output column comes from.
enum Src {
    Col(usize),
    Lit(Cell),
}

struct Query {
    table: String,
    filter: Where,
    order: Vec<(String, bool)>,
    limit: Option<usize>,
    offset: usize,
}

impl SampleServer {
    pub(super) fn select(&self, db: &str, p: &mut Parser, rs: &mut ResultSet) -> Res<()> {
        rs.has_rows = true;
        let items = p.comma_list(|p| select_item(p, db))?;
        if !p.kw("FROM") {
            p.done()?;
            return literal_row(&items, rs);
        }
        let q = parse_query(p)?;
        let tb = self.table(db, &q.table)?;
        let rows = matching_rows(tb, &q)?;
        if items.iter().any(|i| matches!(i, Item::Count(_))) {
            return count_row(&items, rows.len(), rs);
        }
        let srcs = output_columns(tb, &items, rs)?;
        rs.rows = rows.into_iter().skip(q.offset).take(q.limit.unwrap_or(usize::MAX)).map(|r| project(r, &srcs)).collect();
        Ok(())
    }
}

fn select_item(p: &mut Parser, db: &str) -> Res<Item> {
    if p.sym("*") {
        return Ok(Item::Star);
    }
    if p.is_kw("COUNT") && matches!(p.t.get(p.i + 1), Some(Tok::Sym(s)) if s == "(") {
        p.i += 2;
        if !p.sym("*") {
            p.ident()?;
        }
        p.expect_sym(")")?;
        return Ok(Item::Count(p.alias()?.unwrap_or_else(|| "COUNT(*)".into())));
    }
    if matches!(p.peek(), Some(Tok::Str(_)) | Some(Tok::Num(_))) || p.is_kw("NULL") || p.is_kw("NOW") || p.is_kw("CURRENT_TIMESTAMP") {
        let label = p.near().trim_matches('\'').to_string();
        let v = p.literal()?;
        return Ok(Item::Lit(v, p.alias()?.unwrap_or(label)));
    }
    if p.is_kw("VERSION") {
        return function_item(p, "VERSION()", Some("8.0-sample".into()));
    }
    if p.is_kw("DATABASE") {
        return function_item(p, "DATABASE()", Some(db.to_string()));
    }
    let c = p.ident()?;
    if c == "*" {
        return Ok(Item::Star);
    }
    let a = p.alias()?.unwrap_or_else(|| c.clone());
    Ok(Item::Col(c, a))
}

/// `NAME()` with no arguments and a fixed value.
fn function_item(p: &mut Parser, label: &str, value: Cell) -> Res<Item> {
    p.i += 1;
    p.expect_sym("(")?;
    p.expect_sym(")")?;
    Ok(Item::Lit(value, p.alias()?.unwrap_or_else(|| label.into())))
}

/// `SELECT 1, 'x'` without a table.
fn literal_row(items: &[Item], rs: &mut ResultSet) -> Res<()> {
    let mut row = vec![];
    for it in items {
        let Item::Lit(v, a) = it else { return Err(DbError::query("No table used")) };
        rs.columns.push(a.clone());
        row.push(v.clone());
    }
    rs.rows = vec![row];
    Ok(())
}

fn parse_query(p: &mut Parser) -> Res<Query> {
    let table = p.ident()?;
    p.alias()?;
    let filter = parse_where(p)?;
    let mut order = vec![];
    if p.kw("ORDER") {
        p.expect_kw("BY")?;
        order = p.comma_list(order_item)?;
    }
    let (limit, offset) = parse_limit(p)?;
    p.done()?;
    Ok(Query { table, filter, order, limit, offset })
}

fn order_item(p: &mut Parser) -> Res<(String, bool)> {
    let c = p.ident()?;
    let desc = p.kw("DESC");
    if !desc {
        p.kw("ASC");
    }
    Ok((c, desc))
}

/// `LIMIT n`, `LIMIT offset, n` or `LIMIT n OFFSET m`.
fn parse_limit(p: &mut Parser) -> Res<(Option<usize>, usize)> {
    if !p.kw("LIMIT") {
        return Ok((None, 0));
    }
    let a = p.num()?;
    if p.sym(",") {
        return Ok((Some(p.num()?), a));
    }
    let offset = if p.kw("OFFSET") { p.num()? } else { 0 };
    Ok((Some(a), offset))
}

fn matching_rows<'t>(tb: &'t SampleTable, q: &Query) -> Res<Vec<&'t Row>> {
    let mut rows = vec![];
    for r in &tb.rows {
        if row_matches(tb, r, &q.filter)? {
            rows.push(r);
        }
    }
    if q.order.is_empty() {
        return Ok(rows);
    }
    let idx: Vec<(usize, bool)> = q.order.iter().map(|(c, d)| Ok((tb.col_index(c)?, *d))).collect::<Res<_>>()?;
    rows.sort_by(|a, b| compare_rows(a, b, &idx));
    Ok(rows)
}

fn compare_rows(a: &Row, b: &Row, idx: &[(usize, bool)]) -> Ordering {
    for (k, desc) in idx {
        let o = cmp_cells(&a[*k], &b[*k]);
        if !o.is_eq() {
            return if *desc { o.reverse() } else { o };
        }
    }
    Ordering::Equal
}

fn count_row(items: &[Item], count: usize, rs: &mut ResultSet) -> Res<()> {
    let mut row = vec![];
    for it in items {
        match it {
            Item::Count(a) => {
                rs.columns.push(a.clone());
                row.push(Some(count.to_string()));
            }
            Item::Lit(v, a) => {
                rs.columns.push(a.clone());
                row.push(v.clone());
            }
            _ => return Err(DbError::query("Mixing COUNT(*) with columns needs GROUP BY, which the sample database doesn't support")),
        }
    }
    rs.rows = vec![row];
    Ok(())
}

fn output_columns(tb: &SampleTable, items: &[Item], rs: &mut ResultSet) -> Res<Vec<Src>> {
    let mut srcs = vec![];
    for it in items {
        match it {
            Item::Star => {
                rs.columns.extend(tb.fields.iter().map(|f| f.name.clone()));
                srcs.extend((0..tb.fields.len()).map(Src::Col));
            }
            Item::Col(c, a) => {
                srcs.push(Src::Col(tb.col_index(c)?));
                rs.columns.push(a.clone());
            }
            Item::Lit(v, a) => {
                srcs.push(Src::Lit(v.clone()));
                rs.columns.push(a.clone());
            }
            Item::Count(_) => {}
        }
    }
    Ok(srcs)
}

fn project(r: &Row, srcs: &[Src]) -> Row {
    srcs.iter()
        .map(|s| match s {
            Src::Col(k) => r[*k].clone(),
            Src::Lit(v) => v.clone(),
        })
        .collect()
}
