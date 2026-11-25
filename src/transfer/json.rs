//! JSON array of objects, one per row.

use super::Exporter;
use crate::domain::ResultSet;
use crate::sql::Dialect;

pub(super) struct JsonExporter;

fn json_string(s: &str) -> String {
    serde_json::to_string(s).unwrap_or_else(|_| "\"\"".into())
}

impl Exporter for JsonExporter {
    fn render(&self, _table: &str, rs: &ResultSet, _header: bool, _dialect: &dyn Dialect) -> String {
        let mut out = String::from("[\n");
        for (i, r) in rs.rows.iter().enumerate() {
            let fields: Vec<String> = rs
                .columns
                .iter()
                .zip(r)
                .map(|(c, v)| format!("\"{}\": {}", c.replace('"', "\\\""), v.as_deref().map(json_string).unwrap_or_else(|| "null".into())))
                .collect();
            let comma = if i + 1 < rs.rows.len() { "," } else { "" };
            out.push_str(&format!("  {{{}}}{}\n", fields.join(", "), comma));
        }
        out.push_str("]\n");
        out
    }
}
