//! State of a materialized view designer tab (no egui).

use super::super::QueryTab;
use crate::domain::{IndexChoices, MatViewDef};
use crate::sql::{create_matview, Dialect, PgDialect, UNTITLED};

/// The designer's sub tabs.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum MatViewSub {
    #[default]
    Definition,
    Advanced,
    Comment,
    SqlPreview,
}

impl MatViewSub {
    pub const ALL: [MatViewSub; 4] = [MatViewSub::Definition, MatViewSub::Advanced, MatViewSub::Comment, MatViewSub::SqlPreview];

    pub fn label(self) -> &'static str {
        match self {
            MatViewSub::Definition => "Definition",
            MatViewSub::Advanced => "Advanced",
            MatViewSub::Comment => "Comment",
            MatViewSub::SqlPreview => "SQL Preview",
        }
    }
}

pub struct MatViewTab {
    pub id: u64,
    pub cid: u64,
    /// The definition editor, and Preview / Explain results. Its database and schema
    /// are where the view is created; its SQL is the view's query.
    pub query: QueryTab,
    /// Everything else about the view (its `definition` is taken from `query`).
    pub def: MatViewDef,
    pub sub: MatViewSub,
    /// The name it was created under; None until the first save.
    pub created: Option<String>,
    /// The view as last saved.
    saved: Option<MatViewDef>,
    pub saving: bool,
    pub error: Option<String>,
    /// Owners and tablespaces for the Advanced drop-downs.
    pub choices: IndexChoices,
}

impl MatViewTab {
    pub fn new(query: QueryTab) -> Self {
        let def = MatViewDef { with_data: true, ..Default::default() };
        MatViewTab { id: query.id, cid: query.cid, query, def, sub: MatViewSub::default(), created: None, saved: None, saving: false, error: None, choices: IndexChoices::default() }
    }

    /// Materialized views only exist on PostgreSQL.
    pub fn dialect(&self) -> &'static dyn Dialect {
        &PgDialect
    }

    /// The view as it stands in the tab.
    pub fn current(&self) -> MatViewDef {
        MatViewDef { schema: self.query.schema.clone(), definition: self.query.sql.clone(), ..self.def.clone() }
    }

    pub fn shown_name(&self) -> &str {
        match self.def.name.trim() {
            "" => UNTITLED,
            n => n,
        }
    }

    /// What Save runs: CREATE (replacing the saved view once there is one); empty when
    /// nothing changed since the last save.
    pub fn statements(&self) -> Vec<String> {
        let now = self.current();
        if self.saved.as_ref() == Some(&now) {
            return vec![];
        }
        create_matview(self.dialect(), &now, self.created.as_deref())
    }

    pub fn has_unsaved(&self) -> bool {
        let untouched_new = self.created.is_none() && self.query.sql.trim().is_empty();
        !untouched_new && !self.statements().is_empty()
    }

    /// The server took the statements: the view now exists as it is in the tab.
    pub fn saved_ok(&mut self) {
        self.saving = false;
        self.error = None;
        self.created = Some(self.def.name.trim().to_string());
        self.saved = Some(self.current());
        self.query.dirty = false;
    }
}

#[cfg(test)]
mod tests {
    use super::MatViewTab;
    use crate::ui::views::QueryTab;

    fn tab(sql: &str) -> MatViewTab {
        let mut q = QueryTab::new(7, 1, "postgres".into(), sql.into());
        q.schema = "public".into();
        MatViewTab::new(q)
    }

    #[test]
    fn a_new_empty_view_has_nothing_to_lose() {
        let t = tab("");
        assert_eq!((t.shown_name(), t.has_unsaved()), ("Untitled", false));
        assert_eq!(t.statements(), ["CREATE MATERIALIZED VIEW \"public\".\"Untitled\"\nAS\n"]);
    }

    #[test]
    fn after_saving_only_changes_recreate_it() {
        let mut t = tab("SELECT 1");
        t.def.name = "one".into();
        assert!(t.has_unsaved());
        t.saved_ok();
        assert!(t.statements().is_empty() && !t.has_unsaved());
        t.query.sql = "SELECT 2".into();
        let stmts = t.statements();
        assert_eq!(stmts[0], "DROP MATERIALIZED VIEW IF EXISTS \"public\".\"one\"");
        assert!(stmts[1].ends_with("AS\nSELECT 2"), "{}", stmts[1]);
    }
}
