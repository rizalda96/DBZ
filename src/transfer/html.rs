//! A plain HTML table.

use super::Exporter;
use crate::domain::ResultSet;
use crate::sql::Dialect;

pub(super) struct HtmlExporter;

fn escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

fn row(tag: &str, cells: impl Iterator<Item = String>) -> String {
    let inner: String = cells.map(|c| format!("<{tag}>{c}</{tag}>")).collect();
    format!("<tr>{}</tr>\n", inner)
}

impl Exporter for HtmlExporter {
    fn render(&self, table: &str, rs: &ResultSet, header: bool, _dialect: &dyn Dialect) -> String {
        let mut out = format!(
            "<!DOCTYPE html>\n<html><head><meta charset=\"utf-8\"><title>{}</title></head><body>\n<table border=\"1\" cellspacing=\"0\" cellpadding=\"4\">\n",
            escape(table)
        );
        if header {
            out.push_str(&row("th", rs.columns.iter().map(|c| escape(c))));
        }
        for r in &rs.rows {
            out.push_str(&row("td", r.iter().map(|v| v.as_deref().map(escape).unwrap_or_default())));
        }
        out.push_str("</table>\n</body></html>\n");
        out
    }
}
