//! CSV and tab-separated text.

use super::Exporter;
use crate::domain::{Cell, ResultSet};
use crate::sql::Dialect;

/// CSV (quoted when needed) or TSV (tabs and line breaks flattened to spaces).
pub(super) struct DelimitedExporter {
    sep: &'static str,
    quoted: bool,
}

impl DelimitedExporter {
    pub const CSV: Self = DelimitedExporter { sep: ",", quoted: true };
    pub const TSV: Self = DelimitedExporter { sep: "\t", quoted: false };

    fn escape(&self, v: &Cell) -> String {
        if self.quoted {
            csv_escape(v)
        } else {
            v.clone().unwrap_or_default().replace(['\t', '\n'], " ")
        }
    }

    fn line(&self, cells: impl Iterator<Item = String>) -> String {
        format!("{}\r\n", cells.collect::<Vec<_>>().join(self.sep))
    }
}

impl Exporter for DelimitedExporter {
    fn render(&self, _table: &str, rs: &ResultSet, header: bool, _dialect: &dyn Dialect) -> String {
        let mut out = String::new();
        if header {
            out.push_str(&self.line(rs.columns.iter().map(|c| self.escape(&Some(c.clone())))));
        }
        for r in &rs.rows {
            out.push_str(&self.line(r.iter().map(|v| self.escape(v))));
        }
        out
    }
}

fn csv_escape(v: &Cell) -> String {
    let Some(s) = v else { return String::new() };
    if s.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.clone()
    }
}

/// Parses delimited text into rows of fields. Handles quoted fields with doubled quotes.
pub fn parse_csv(text: &str, delim: char) -> Vec<Vec<String>> {
    let mut rows = Vec::new();
    let mut row = Vec::new();
    let mut field = String::new();
    let mut in_quotes = false;
    let mut chars = text.chars().peekable();
    let mut any = false;
    while let Some(c) = chars.next() {
        any = true;
        if in_quotes {
            match c {
                '"' if chars.peek() == Some(&'"') => {
                    field.push('"');
                    chars.next();
                }
                '"' => in_quotes = false,
                _ => field.push(c),
            }
            continue;
        }
        match c {
            '"' => in_quotes = true,
            '\n' => {
                row.push(std::mem::take(&mut field));
                rows.push(std::mem::take(&mut row));
                any = false;
            }
            '\r' => {}
            c if c == delim => row.push(std::mem::take(&mut field)),
            _ => field.push(c),
        }
    }
    if any {
        row.push(field);
        rows.push(row);
    }
    rows.retain(|r| !(r.len() == 1 && r[0].is_empty()));
    rows
}
