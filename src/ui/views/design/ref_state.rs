//! Columns of the tables this design's foreign keys reference (no egui). Each table's
//! columns are asked of the server once, the first time a row needs them, then kept.

use std::collections::{HashMap, HashSet};

/// (schema or database, table).
pub type TableKey = (String, String);

#[derive(Default)]
pub struct RefColumns {
    loaded: HashMap<TableKey, Vec<String>>,
    asked: HashSet<TableKey>,
    /// Tables to ask the server about; the view turns them into requests.
    outbox: Vec<TableKey>,
}

fn key(schema: &str, table: &str) -> TableKey {
    (schema.trim().to_string(), table.trim().to_string())
}

impl RefColumns {
    /// Makes sure `table`'s columns are on their way (or here).
    pub fn want(&mut self, schema: &str, table: &str) {
        let k = key(schema, table);
        if !k.1.is_empty() && !self.loaded.contains_key(&k) && self.asked.insert(k.clone()) {
            self.outbox.push(k);
        }
    }

    /// `table`'s columns; empty until they arrive.
    pub fn get(&self, schema: &str, table: &str) -> Vec<String> {
        self.loaded.get(&key(schema, table)).cloned().unwrap_or_default()
    }

    pub fn take_requests(&mut self) -> Vec<TableKey> {
        std::mem::take(&mut self.outbox)
    }

    /// The server's answer. A failed one is stored empty, so it isn't asked again
    /// every frame.
    pub fn arrived(&mut self, schema: &str, table: &str, columns: Vec<String>) {
        self.loaded.insert(key(schema, table), columns);
    }
}

#[cfg(test)]
mod tests {
    use super::RefColumns;

    #[test]
    fn each_table_is_asked_for_once() {
        let mut r = RefColumns::default();
        r.want("public", "master_site");
        r.want("public", "master_site ");
        r.want("public", "");
        assert_eq!(r.take_requests(), [("public".to_string(), "master_site".to_string())]);
        assert!(r.get("public", "master_site").is_empty());
        r.arrived("public", "master_site", vec!["id".into(), "name".into()]);
        r.want("public", "master_site");
        assert!(r.take_requests().is_empty());
        assert_eq!(r.get("public", "master_site"), ["id", "name"]);
    }
}
