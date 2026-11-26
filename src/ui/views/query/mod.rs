//! The SQL query editor tab.

mod editor;
mod results;
mod state;
mod toolbar;

pub use state::QueryTab;

use eframe::egui::{self, Ui};

use crate::app::{Act, ConnState};
use crate::worker::Req;

pub fn query_view(ui: &mut Ui, t: &mut QueryTab, conns: &[ConnState], acts: &mut Vec<Act>) {
    let run_key = ui.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::R) || i.consume_key(egui::Modifiers::NONE, egui::Key::F9));
    let save_key = ui.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::S));
    if save_key {
        acts.push(Act::SaveQuery(t.id));
    }
    let wanted = toolbar::show(ui, t, conns, acts);
    let explain = wanted.explain;
    if (run_key || wanted.run || explain) && !t.running {
        start_run(t, explain, acts);
    }
    editor_with_results(ui, t);
}

/// The SQL editor with the result panel under it; also the materialized view
/// designer's Definition page.
pub fn editor_with_results(ui: &mut Ui, t: &mut QueryTab) {
    results::panel(ui, t);
    editor::panel(ui, t);
}

/// Runs the editor's SQL (or its selection), as EXPLAIN when asked; results land in
/// `t` when the worker answers.
pub fn start_run(t: &mut QueryTab, explain: bool, acts: &mut Vec<Act>) {
    let Some(sql) = t.sql_to_run() else {
        acts.push(Act::Status("Nothing to run".into()));
        return;
    };
    t.running = true;
    t.error = None;
    acts.push(Act::Send(t.cid, Req::Query { tab: t.id, db: t.db.clone(), schema: t.schema.clone(), sql, explain }));
}
