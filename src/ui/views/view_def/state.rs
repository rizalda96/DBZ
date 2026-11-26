//! State of a Design View tab (no egui): the view's query in an editor, and the
//! header that saves it, which the user doesn't see.

use super::super::QueryTab;
use crate::sql::view_statement;

pub struct ViewTab {
    pub id: u64,
    pub cid: u64,
    /// The view as the tree names it.
    pub name: String,
    /// The query editor and Preview results.
    pub query: QueryTab,
    /// CREATE OR REPLACE / ALTER … AS, from the server's definition.
    head: String,
    /// The query as last saved.
    saved: String,
    pub saving: bool,
    pub error: Option<String>,
}

impl ViewTab {
    pub fn new(query: QueryTab, name: String, head: String) -> Self {
        let saved = query.sql.clone();
        ViewTab { id: query.id, cid: query.cid, name, query, head, saved, saving: false, error: None }
    }

    /// What Run sends to change the view.
    pub fn statement(&self) -> String {
        view_statement(&self.head, &self.query.sql)
    }

    pub fn has_unsaved(&self) -> bool {
        self.query.sql.trim() != self.saved.trim()
    }

    /// The server took the new query.
    pub fn saved_ok(&mut self) {
        self.saving = false;
        self.error = None;
        self.saved = self.query.sql.clone();
        self.query.dirty = false;
    }
}

#[cfg(test)]
mod tests {
    use super::ViewTab;
    use crate::ui::views::QueryTab;

    #[test]
    fn run_saves_the_edited_query_under_the_header() {
        let q = QueryTab::new(3, 1, "shop".into(), "SELECT 1".into());
        let mut t = ViewTab::new(q, "v".into(), "ALTER VIEW [dbo].[v] AS".into());
        assert!(!t.has_unsaved());
        t.query.sql = "SELECT 2;".into();
        assert!(t.has_unsaved());
        assert_eq!(t.statement(), "ALTER VIEW [dbo].[v] AS\nSELECT 2");
        t.saved_ok();
        assert!(!t.has_unsaved());
    }
}
