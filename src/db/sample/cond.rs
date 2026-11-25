//! WHERE clauses: parsing and row matching.

use std::cmp::Ordering;

use super::parser::{Parser, Res};
use super::server::SampleTable;
use super::tokens::Tok;
use crate::db::DbError;
use crate::domain::{Cell, Row};

const COMPARISONS: [&str; 7] = ["=", "<", ">", "<=", ">=", "<>", "!="];

pub(super) enum Cond {
    Cmp(String, String, Cell),
    IsNull(String, bool),
    In(String, Vec<Cell>),
}

/// OR of ANDs. Empty when there is no WHERE.
pub(super) type Where = Vec<Vec<Cond>>;

pub(super) fn parse_where(p: &mut Parser) -> Res<Where> {
    let mut ors = vec![];
    if !p.kw("WHERE") {
        return Ok(ors);
    }
    loop {
        let mut ands = vec![parse_condition(p)?];
        while p.kw("AND") {
            ands.push(parse_condition(p)?);
        }
        ors.push(ands);
        if !p.kw("OR") {
            return Ok(ors);
        }
    }
}

fn parse_condition(p: &mut Parser) -> Res<Cond> {
    let paren = p.sym("(");
    let col = p.ident()?;
    let cond = if p.kw("IS") {
        let not = p.kw("NOT");
        p.expect_kw("NULL")?;
        Cond::IsNull(col, !not)
    } else if p.kw("IN") {
        p.expect_sym("(")?;
        let vals = p.comma_list(|p| p.literal())?;
        p.expect_sym(")")?;
        Cond::In(col, vals)
    } else {
        let op = comparison(p)?;
        Cond::Cmp(col, op, p.literal()?)
    };
    if paren {
        p.expect_sym(")")?;
    }
    Ok(cond)
}

fn comparison(p: &mut Parser) -> Res<String> {
    if p.kw("LIKE") {
        return Ok("LIKE".into());
    }
    if p.kw("NOT") {
        p.expect_kw("LIKE")?;
        return Ok("NOT LIKE".into());
    }
    match p.peek().cloned() {
        Some(Tok::Sym(s)) if COMPARISONS.contains(&s.as_str()) => {
            p.i += 1;
            Ok(s)
        }
        _ => Err(DbError::query(format!("Expected a comparison near {}", p.near()))),
    }
}

/// NULL sorts first; numbers compare as numbers, text case-insensitively.
pub(super) fn cmp_cells(a: &Cell, b: &Cell) -> Ordering {
    match (a, b) {
        (None, None) => Ordering::Equal,
        (None, _) => Ordering::Less,
        (_, None) => Ordering::Greater,
        (Some(x), Some(y)) => match (x.trim().parse::<f64>(), y.trim().parse::<f64>()) {
            (Ok(fx), Ok(fy)) => fx.partial_cmp(&fy).unwrap_or(Ordering::Equal),
            _ => x.to_lowercase().cmp(&y.to_lowercase()),
        },
    }
}

/// SQL LIKE with `%` and `_`, case-insensitive.
fn like(text: &str, pat: &str) -> bool {
    fn rec(t: &[char], p: &[char]) -> bool {
        match p.first() {
            None => t.is_empty(),
            Some('%') => (0..=t.len()).any(|k| rec(&t[k..], &p[1..])),
            Some('_') => !t.is_empty() && rec(&t[1..], &p[1..]),
            Some(c) => !t.is_empty() && t[0].to_lowercase().eq(c.to_lowercase()) && rec(&t[1..], &p[1..]),
        }
    }
    let t: Vec<char> = text.chars().collect();
    let p: Vec<char> = pat.chars().collect();
    rec(&t, &p)
}

pub(super) fn row_matches(t: &SampleTable, row: &Row, ors: &Where) -> Res<bool> {
    if ors.is_empty() {
        return Ok(true);
    }
    for ands in ors {
        if all_hold(t, row, ands)? {
            return Ok(true);
        }
    }
    Ok(false)
}

fn all_hold(t: &SampleTable, row: &Row, ands: &[Cond]) -> Res<bool> {
    for c in ands {
        if !holds(t, row, c)? {
            return Ok(false);
        }
    }
    Ok(true)
}

fn holds(t: &SampleTable, row: &Row, c: &Cond) -> Res<bool> {
    Ok(match c {
        Cond::IsNull(col, want_null) => row[t.col_index(col)?].is_none() == *want_null,
        Cond::In(col, vals) => {
            let v = &row[t.col_index(col)?];
            v.is_some() && vals.iter().any(|x| cmp_cells(v, x).is_eq())
        }
        Cond::Cmp(col, op, val) => compare(&row[t.col_index(col)?], op, val),
    })
}

fn compare(v: &Cell, op: &str, val: &Cell) -> bool {
    let (Some(text), Some(pat)) = (v.as_deref(), val.as_deref()) else { return false };
    let o = cmp_cells(v, val);
    match op {
        "=" => o.is_eq(),
        "<>" | "!=" => !o.is_eq(),
        "<" => o.is_lt(),
        ">" => o.is_gt(),
        "<=" => o.is_le(),
        ">=" => o.is_ge(),
        "LIKE" => like(text, pat),
        "NOT LIKE" => !like(text, pat),
        _ => false,
    }
}
