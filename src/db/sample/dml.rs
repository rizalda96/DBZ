//! INSERT, UPDATE, DELETE and TRUNCATE.

use super::cond::{cmp_cells, parse_where, row_matches};
use super::parser::{Parser, Res};
use super::server::{SampleServer, SampleTable};
use crate::db::DbError;
use crate::domain::{Cell, Row};
use crate::shared::now_string;

impl SampleServer {
    pub(super) fn insert(&mut self, db: &str, p: &mut Parser) -> Res<u64> {
        p.kw("INTO");
        let t = p.ident()?;
        let cols = if p.is_sym("(") { Some(p.ident_list()?) } else { None };
        if !p.kw("VALUES") {
            p.expect_kw("VALUE")?;
        }
        let tuples = p.comma_list(value_tuple)?;
        p.done()?;
        let tb = self.table_mut(db, &t)?;
        let idx: Vec<usize> = match &cols {
            Some(c) => c.iter().map(|n| tb.col_index(n)).collect::<Res<_>>()?,
            None => (0..tb.fields.len()).collect(),
        };
        let mut new_rows: Vec<Row> = vec![];
        for vals in tuples {
            let row = build_row(tb, &idx, vals)?;
            check_unique(tb, &new_rows, &row)?;
            new_rows.push(row);
        }
        let n = new_rows.len() as u64;
        tb.rows.extend(new_rows);
        tb.modified = now_string();
        Ok(n)
    }

    pub(super) fn update(&mut self, db: &str, p: &mut Parser) -> Res<u64> {
        let t = p.ident()?;
        p.expect_kw("SET")?;
        let sets = p.comma_list(|p| {
            let c = p.ident()?;
            p.expect_sym("=")?;
            Ok((c, p.literal()?))
        })?;
        let w = parse_where(p)?;
        let limit = parse_limit(p)?;
        p.done()?;
        let tb = self.table_mut(db, &t)?;
        let sets: Vec<(usize, Cell)> = sets.into_iter().map(|(c, v)| Ok((tb.col_index(&c)?, v))).collect::<Res<_>>()?;
        if let Some((k, _)) = sets.iter().find(|(k, v)| tb.fields[*k].not_null && v.is_none()) {
            return Err(DbError::query(format!("Column '{}' cannot be null", tb.fields[*k].name)));
        }
        let mut n = 0;
        for r in 0..tb.rows.len() {
            if limit.is_some_and(|l| n >= l) {
                break;
            }
            if !row_matches(tb, &tb.rows[r], &w)? {
                continue;
            }
            for (k, v) in &sets {
                tb.rows[r][*k] = v.clone();
            }
            n += 1;
        }
        tb.modified = now_string();
        Ok(n as u64)
    }

    pub(super) fn delete(&mut self, db: &str, p: &mut Parser) -> Res<u64> {
        p.expect_kw("FROM")?;
        let t = p.ident()?;
        let w = parse_where(p)?;
        let limit = parse_limit(p)?;
        p.done()?;
        let tb = self.table_mut(db, &t)?;
        let mut keep = vec![];
        let mut n = 0;
        for r in std::mem::take(&mut tb.rows) {
            if limit.map(|l| n < l).unwrap_or(true) && row_matches(tb, &r, &w)? {
                n += 1;
            } else {
                keep.push(r);
            }
        }
        tb.rows = keep;
        tb.modified = now_string();
        Ok(n as u64)
    }

    pub(super) fn truncate(&mut self, db: &str, p: &mut Parser) -> Res<()> {
        p.kw("TABLE");
        let t = p.ident()?;
        p.done()?;
        let tb = self.table_mut(db, &t)?;
        tb.rows.clear();
        tb.auto_inc = 0;
        Ok(())
    }
}

fn parse_limit(p: &mut Parser) -> Res<Option<usize>> {
    if p.kw("LIMIT") {
        return Ok(Some(p.num()?));
    }
    Ok(None)
}

/// `(v1, v2, ...)`
fn value_tuple(p: &mut Parser) -> Res<Vec<Cell>> {
    p.expect_sym("(")?;
    let vals = p.comma_list(|p| p.literal())?;
    p.expect_sym(")")?;
    Ok(vals)
}

/// A new row from defaults plus the given values, with auto-increment filled in.
fn build_row(tb: &mut SampleTable, idx: &[usize], vals: Vec<Cell>) -> Res<Row> {
    if vals.len() != idx.len() {
        return Err(DbError::query("Column count doesn't match value count"));
    }
    let mut row = tb.default_row();
    for (k, v) in idx.iter().zip(vals) {
        row[*k] = v;
    }
    for k in 0..tb.fields.len() {
        if tb.fields[k].auto_inc {
            fill_auto_inc(&mut tb.auto_inc, &mut row[k]);
        }
        if tb.fields[k].not_null && row[k].is_none() {
            return Err(DbError::query(format!("Field '{}' doesn't have a default value", tb.fields[k].name)));
        }
    }
    Ok(row)
}

fn fill_auto_inc(counter: &mut u64, cell: &mut Cell) {
    match cell {
        None => {
            *counter += 1;
            *cell = Some(counter.to_string());
        }
        Some(v) => {
            if let Ok(n) = v.parse::<u64>() {
                *counter = (*counter).max(n);
            }
        }
    }
}

fn check_unique(tb: &SampleTable, pending: &[Row], row: &Row) -> Res<()> {
    let keys: Vec<usize> = tb.fields.iter().enumerate().filter(|(_, f)| f.key).map(|(i, _)| i).collect();
    if keys.is_empty() {
        return Ok(());
    }
    let same_key = |r: &Row| keys.iter().all(|&k| cmp_cells(&r[k], &row[k]).is_eq());
    if tb.rows.iter().chain(pending.iter()).any(same_key) {
        return Err(DbError::query(format!("Duplicate entry for key 'PRIMARY' in '{}'", tb.name)));
    }
    Ok(())
}
