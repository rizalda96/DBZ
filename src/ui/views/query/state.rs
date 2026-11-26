//! State of a query tab.

use std::time::Duration;

use crate::domain::ResultSet;

const HINT: &str = "Write SQL above and press Run (Ctrl+R). Select part of the text to run just that part.";

pub struct QueryTab {
    pub id: u64,
    pub cid: u64,
    pub db: String,
    /// Where unqualified names resolve first (PostgreSQL); empty = the server's default.
    pub schema: String,
    pub title: String,
    pub sql: String,
    pub saved_name: Option<String>,
    pub results: Vec<ResultSet>,
    /// (statement, message) of the statement that failed.
    pub error: Option<(String, String)>,
    pub running: bool,
    pub elapsed: Option<Duration>,
    /// 0 = Message tab, i = results[i - 1].
    pub view: usize,
    pub sel_cell: Option<(usize, usize)>,
    pub dirty: bool,
    /// Editor selection as char indices.
    pub selection: Option<(usize, usize)>,
}

impl QueryTab {
    pub fn new(id: u64, cid: u64, db: String, sql: String) -> Self {
        QueryTab {
            id,
            cid,
            db,
            schema: String::new(),
            title: "Untitled".into(),
            sql,
            saved_name: None,
            results: vec![],
            error: None,
            running: false,
            elapsed: None,
            view: 0,
            sel_cell: None,
            dirty: false,
            selection: None,
        }
    }

    pub fn has_selection(&self) -> bool {
        self.selection.is_some_and(|(a, b)| a != b)
    }

    /// The selected text (or everything); None when blank.
    pub fn sql_to_run(&self) -> Option<String> {
        let sql: String = match self.selection {
            Some((a, b)) if a != b => self.sql.chars().skip(a.min(b)).take(a.abs_diff(b)).collect(),
            _ => self.sql.clone(),
        };
        (!sql.trim().is_empty()).then_some(sql)
    }

    /// The Message tab: each statement with its outcome, then the error and time.
    pub fn message_text(&self) -> String {
        let mut msg = String::new();
        for r in &self.results {
            let outcome = if r.has_rows { format!("> OK, {} row(s) returned", r.rows.len()) } else { format!("> Affected rows: {}", r.affected) };
            msg.push_str(&format!("{}\n{}\n\n", r.statement, outcome));
        }
        if let Some((st, e)) = &self.error {
            msg.push_str(&format!("{}\n> {}\n", st, e));
        }
        if let Some(e) = self.elapsed {
            msg.push_str(&format!("> Time: {:.3}s\n", e.as_secs_f64()));
        }
        if msg.is_empty() {
            msg = HINT.into();
        }
        msg
    }

    /// Rows in the result tab being shown (0 on the Message tab).
    pub fn shown_rows(&self) -> usize {
        self.view.checked_sub(1).and_then(|i| self.results.get(i)).map(|r| r.rows.len()).unwrap_or(0)
    }
}
