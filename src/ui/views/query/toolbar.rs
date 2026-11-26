//! Query tab toolbar: save, connection/database/schema pickers, run, explain, format.

use eframe::egui::{self, Ui};

use super::QueryTab;
use crate::app::{Act, ConnState, DbState};
use crate::sql::{beautify, dialect_for};
use crate::worker::Req;
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
            db_picker(ui, t, conns, acts);
            schema_picker(ui, t, conns);
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
                let first = c.dbs.first();
                t.db = first.map(|d| d.name.clone()).unwrap_or_default();
                t.schema = first.map(DbState::default_schema).unwrap_or_default();
                acts.push(Act::EnsureOpen(c.cfg.id));
            }
        }
    });
}

fn db_picker(ui: &mut Ui, t: &mut QueryTab, conns: &[ConnState], acts: &mut Vec<Act>) {
    let dbs: &[DbState] = conns.iter().find(|c| c.cfg.id == t.cid).map(|c| c.dbs.as_slice()).unwrap_or_default();
    let shown = if t.db.is_empty() { "(database)".to_string() } else { t.db.clone() };
    egui::ComboBox::from_id_salt(("qdb", t.id)).selected_text(shown).width(140.0).show_ui(ui, |ui| {
        for d in dbs {
            if ui.selectable_label(d.name == t.db, &d.name).clicked() {
                pick_db(t, d, acts);
            }
        }
    });
}

/// A new database starts in its default schema; when its schemas aren't loaded yet
/// they are asked for, and the app fills the schema in when they arrive.
fn pick_db(t: &mut QueryTab, d: &DbState, acts: &mut Vec<Act>) {
    t.db = d.name.clone();
    t.schema = d.default_schema();
    if d.schemas.is_empty() {
        acts.push(Act::Send(t.cid, Req::Schemas(d.name.clone())));
    }
}

/// Only on servers with schemas (the list stays empty elsewhere) that can make one the
/// default for a query; SQL Server has schemas but no search path.
fn schema_picker(ui: &mut Ui, t: &mut QueryTab, conns: &[ConnState]) {
    let conn = conns.iter().find(|c| c.cfg.id == t.cid);
    let schemas: &[String] = conn.and_then(|c| c.db(&t.db)).map(|d| d.schemas.as_slice()).unwrap_or_default();
    if schemas.is_empty() && t.schema.is_empty() {
        return;
    }
    let probe = schemas.first().unwrap_or(&t.schema);
    if conn.is_some_and(|c| dialect_for(c.cfg.kind).use_schema(probe).is_none()) {
        return;
    }
    let shown = if t.schema.is_empty() { "(schema)".to_string() } else { t.schema.clone() };
    egui::ComboBox::from_id_salt(("qschema", t.id)).selected_text(shown).width(140.0).show_ui(ui, |ui| {
        for s in schemas {
            ui.selectable_value(&mut t.schema, s.clone(), s);
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
