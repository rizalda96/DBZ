//! Query results as plain text cells.

use std::cmp::Ordering;

/// One value; `None` is SQL NULL.
pub type Cell = Option<String>;
pub type Row = Vec<Cell>;

#[derive(Clone, Debug, Default)]
pub struct ResultSet {
    pub columns: Vec<String>,
    pub rows: Vec<Row>,
    pub affected: u64,
    /// The statement that produced this set (for the Message tab).
    pub statement: String,
    pub has_rows: bool,
}

/// First column of the first row of the first set, e.g. the answer to `SELECT COUNT(*)`.
pub fn first_cell(sets: &[ResultSet]) -> Option<String> {
    sets.first()?.rows.first()?.first()?.clone()
}

/// Order of two values when sorting a result: NULL first, numbers by value, other
/// text case-insensitively (dates in ISO form sort by date that way too).
pub fn compare_cells(a: &Cell, b: &Cell) -> Ordering {
    match (a, b) {
        (None, None) => Ordering::Equal,
        (None, Some(_)) => Ordering::Less,
        (Some(_), None) => Ordering::Greater,
        (Some(x), Some(y)) => match (x.trim().parse::<f64>(), y.trim().parse::<f64>()) {
            (Ok(p), Ok(q)) => p.total_cmp(&q),
            _ => x.to_lowercase().cmp(&y.to_lowercase()).then_with(|| x.cmp(y)),
        },
    }
}

/// Sorts `rows` by column `col`; rows with equal values keep their order.
pub fn sort_rows(rows: &mut [Row], col: usize, desc: bool) {
    rows.sort_by(|a, b| {
        let order = compare_cells(a.get(col).unwrap_or(&None), b.get(col).unwrap_or(&None));
        if desc {
            order.reverse()
        } else {
            order
        }
    });
}

#[cfg(test)]
mod tests {
    use super::{sort_rows, Row};

    fn rows(values: &[Option<&str>]) -> Vec<Row> {
        values.iter().enumerate().map(|(i, v)| vec![Some(i.to_string()), v.map(str::to_string)]).collect()
    }

    fn column(rows: &[Row]) -> Vec<Option<&str>> {
        rows.iter().map(|r| r[1].as_deref()).collect()
    }

    #[test]
    fn numbers_sort_by_value_not_text() {
        let mut r = rows(&[Some("148"), Some("22"), Some("3525.57"), Some("-1")]);
        sort_rows(&mut r, 1, false);
        assert_eq!(column(&r), [Some("-1"), Some("22"), Some("148"), Some("3525.57")]);
        sort_rows(&mut r, 1, true);
        assert_eq!(column(&r), [Some("3525.57"), Some("148"), Some("22"), Some("-1")]);
    }

    #[test]
    fn nulls_first_and_text_ignores_case() {
        let mut r = rows(&[Some("keban"), None, Some("BRAM"), Some("2026-06-01")]);
        sort_rows(&mut r, 1, false);
        assert_eq!(column(&r), [None, Some("2026-06-01"), Some("BRAM"), Some("keban")]);
    }

    #[test]
    fn equal_values_keep_their_order() {
        let mut r = rows(&[Some("a"), Some("b"), Some("a")]);
        sort_rows(&mut r, 1, false);
        let ids: Vec<&str> = r.iter().map(|row| row[0].as_deref().unwrap_or_default()).collect();
        assert_eq!(ids, ["0", "2", "1"]);
    }
}
