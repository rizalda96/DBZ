//! Query tab toolbar: save, connection/database pickers, run, explain, format.

use eframe::egui::{self, Ui};

use super::QueryTab;
use crate::app::{Act, ConnState};
use crate::sql::beautify;
use crate::ui::icons::Icon;
use crate::ui::widgets::{tool_btn, tool_sep, toolbar_frame};

/// Buttons pressed this frame that the caller acts on.
#[derive(Default)]
pub(super) struct Wanted {
    pub run: bool,
    pub explain: bool,
}

pub(super) fn show(ui: &mut Ui, t: &mut QueryTab, conns: &[ConnState], acts: &mut Vec<Act>) -> Wanted {
    let mut wanted = Wanted::default();
    toolbar_frame().show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 2.0;
            if tool_btn(ui, Some(Icon::Save), "Save", true).clicked() {
                acts.push(Act::SaveQuery(t.id));
            }
            tool_sep(ui);
            conn_picker(ui, t, conns, acts);
            db_picker(ui, t, conns);
            tool_sep(ui);
            run_buttons(ui, t, &mut wanted);
            tool_sep(ui);
            edit_buttons(ui, t);
            if t.running {
                ui.add_space(8.0);
                ui.spinner();
                ui.label("Running…");
            }
        });
    });
    wanted
}

fn conn_picker(ui: &mut Ui, t: &mut QueryTab, conns: &[ConnState], acts: &mut Vec<Act>) {
    let cname = conns.iter().find(|c| c.cfg.id == t.cid).map(|c| c.cfg.name.clone()).unwrap_or_else(|| "(none)".into());
    egui::ComboBox::from_id_salt(("qconn", t.id)).selected_text(cname).width(150.0).show_ui(ui, |ui| {
        for c in conns {
            if ui.selectable_label(c.cfg.id == t.cid, &c.cfg.name).clicked() {
                t.cid = c.cfg.id;
                t.db = c.dbs.first().map(|d| d.name.clone()).unwrap_or_default();
                acts.push(Act::EnsureOpen(c.cfg.id));
            }
        }
    });
}

fn db_picker(ui: &mut Ui, t: &mut QueryTab, conns: &[ConnState]) {
    let dbs: Vec<String> = conns.iter().find(|c| c.cfg.id == t.cid).map(|c| c.dbs.iter().map(|d| d.name.clone()).collect()).unwrap_or_default();
    let shown = if t.db.is_empty() { "(database)".to_string() } else { t.db.clone() };
    egui::ComboBox::from_id_salt(("qdb", t.id)).selected_text(shown).width(140.0).show_ui(ui, |ui| {
        for d in dbs {
            if ui.selectable_label(d == t.db, &d).clicked() {
                t.db = d;
            }
        }
    });
}

fn run_buttons(ui: &mut Ui, t: &QueryTab, wanted: &mut Wanted) {
    let label = if t.has_selection() { "Run Selected" } else { "Run" };
    wanted.run = tool_btn(ui, Some(Icon::Run), label, !t.running).on_hover_text("Ctrl+R").clicked();
    tool_btn(ui, None, "Stop", false);
    wanted.explain = tool_btn(ui, None, "Explain", !t.running).clicked();
}

fn edit_buttons(ui: &mut Ui, t: &mut QueryTab) {
    if tool_btn(ui, None, "Beautify SQL", true).clicked() {
        t.sql = beautify(&t.sql);
        t.dirty = true;
    }
    if tool_btn(ui, None, "Clear", true).clicked() {
        t.sql.clear();
        t.dirty = true;
    }
}
