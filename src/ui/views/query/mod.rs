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
    results::panel(ui, t);
    editor::panel(ui, t);
}

fn start_run(t: &mut QueryTab, explain: bool, acts: &mut Vec<Act>) {
    let Some(sql) = t.sql_to_run(explain) else {
        acts.push(Act::Status("Nothing to run".into()));
        return;
    };
    t.running = true;
    t.error = None;
    acts.push(Act::Send(t.cid, Req::Query { tab: t.id, db: t.db.clone(), sql }));
}
