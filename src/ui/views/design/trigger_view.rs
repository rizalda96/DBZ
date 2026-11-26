//! The designer's Triggers tab: the grid (Update of Fields picked in a tick list) and
//! the panel under it. On PostgreSQL the panel has General (WHEN, function, arguments)
//! and Constraint (CONSTRAINT TRIGGER, DEFERRABLE, FROM table) pages; on MySQL it
//! holds the statement the trigger runs.

use eframe::egui::{self, Rect, RichText, Ui};

use super::form_rows::{body_panel, deferrable_rows, page_tabs, panel_frame, PanelText, FIELD_WIDTH};
use super::trigger_options_view;
use crate::ui::widgets::search_combo;
use super::parts_grid::{grid, Cell, GridSpec};
use super::parts_state::PartKind;
use super::DesignTab;
use crate::domain::{IndexChoices, TriggerDef};
use crate::sql::DesignerCaps;
use crate::ui::theme::pal;

const FOR_EACH: [&str; 2] = ["ROW", "STATEMENT"];
const COLUMNS: [(&str, f32); 10] = [
    ("Name", 180.0),
    ("For Each", 100.0),
    ("Fires", 100.0),
    ("Insert", 56.0),
    ("Update", 56.0),
    ("Delete", 56.0),
    ("Truncate", 64.0),
    ("Update of Fields", 140.0),
    ("Enable", 56.0),
    ("Comment", 140.0),
];
/// Widths of a schema + name pair of drop-downs.
const SCHEMA_WIDTH: f32 = 180.0;
const NAME_WIDTH: f32 = 230.0;

/// Returns the row whose Update of Fields "…" was clicked, with the cell's rect.
/// Columns the server doesn't have are left out.
pub(super) fn trigger_grid(ui: &mut Ui, (id, sel): ((&'static str, u64), &mut Option<usize>), triggers: &mut [TriggerDef], caps: &'static DesignerCaps) -> Option<(usize, Rect)> {
    let shown = shown_columns(caps);
    let columns: Vec<(&str, f32)> = COLUMNS.iter().zip(shown).filter(|(_, on)| *on).map(|(c, _)| *c).collect();
    let before: Vec<[bool; 3]> = triggers.iter().map(events).collect();
    let opened = grid(ui, &GridSpec { id, columns: &columns }, triggers, sel, |t: &mut TriggerDef| {
        trigger_cells(t, caps).into_iter().zip(shown).filter(|(_, on)| *on).map(|(c, _)| c).collect::<Vec<_>>()
    });
    if caps.trigger_one_event {
        triggers.iter_mut().zip(before).for_each(|(t, was)| keep_one_event(t, was));
    }
    opened
}

/// Which of `COLUMNS` the server has.
fn shown_columns(caps: &DesignerCaps) -> [bool; COLUMNS.len()] {
    let extra = caps.trigger_extras;
    [true, extra, true, true, true, true, extra, extra, caps.trigger_enable, caps.trigger_comments]
}

fn trigger_cells<'b>(t: &'b mut TriggerDef, caps: &DesignerCaps) -> Vec<Cell<'b>> {
    vec![
        Cell::Text(&mut t.name),
        Cell::Choice(&mut t.for_each, &FOR_EACH),
        Cell::Choice(&mut t.fires, caps.trigger_timings),
        Cell::Check(&mut t.insert),
        Cell::Check(&mut t.update),
        Cell::Check(&mut t.delete),
        Cell::Check(&mut t.truncate),
        Cell::Editor(&t.update_of),
        Cell::Check(&mut t.enabled),
        Cell::Text(&mut t.comment),
    ]
}

fn events(t: &TriggerDef) -> [bool; 3] {
    [t.insert, t.update, t.delete]
}

/// Servers whose triggers have one event: ticking another event moves the tick.
fn keep_one_event(t: &mut TriggerDef, was: [bool; 3]) {
    let now = events(t);
    if now.iter().filter(|e| **e).count() <= 1 {
        return;
    }
    let newly = (0..3).find(|i| now[*i] && !was[*i]).unwrap_or(0);
    let only = |i: usize| i == newly;
    (t.insert, t.update, t.delete) = (only(0), only(1), only(2));
}

/// The panel's pages (PostgreSQL).
#[derive(Clone, Copy, Default, PartialEq, Eq)]
enum Page {
    #[default]
    General,
    Constraint,
}

/// The panel under the grid for the selected trigger.
pub(super) fn panel(ui: &mut Ui, t: &mut DesignTab) {
    let caps = t.dialect().designer();
    let id = t.id;
    let sel = t.parts.sel(PartKind::Trigger);
    let trigger = sel.and_then(|i| t.parts.parts.triggers.get_mut(i));
    if caps.trigger_options {
        return trigger_options_view::panel(ui, id, trigger);
    }
    if !caps.trigger_extras {
        let text = PanelText { label: caps.trigger_body, hint: "SET NEW.updated_at = NOW()", empty: "Select a trigger to edit what it runs." };
        return body_panel(ui, ("dtrigger", id), &text, trigger.map(|tr| &mut tr.body));
    }
    let choices = &t.index_choices;
    egui::Panel::bottom(("dtrigger", id)).resizable(true).default_size(150.0).frame(panel_frame()).show(ui, |ui| {
        let Some(tr) = trigger else {
            ui.label(RichText::new("Select a trigger to edit what it runs.").color(pal().muted));
            return;
        };
        let page = page_tabs(ui, ("dtrigger_page", id), &[(Page::General, "General"), (Page::Constraint, "Constraint")]);
        egui::Grid::new(("dtrigger_opts", id)).num_columns(2).spacing([24.0, 8.0]).min_col_width(140.0).show(ui, |ui| match page {
            Page::General => general(ui, id, choices, tr),
            Page::Constraint => constraint(ui, id, choices, tr),
        });
    });
}

fn general(ui: &mut Ui, id: u64, choices: &IndexChoices, tr: &mut TriggerDef) {
    ui.label("When:");
    ui.add(egui::TextEdit::singleline(&mut tr.when).desired_width(FIELD_WIDTH).hint_text("new.status <> old.status"));
    ui.end_row();
    ui.label("Trigger Function:");
    ui.horizontal(|ui| {
        search_combo(ui, (("dtrigger_fn_schema", id), SCHEMA_WIDTH), &mut tr.function_schema, &choices.function_schemas());
        search_combo(ui, (("dtrigger_fn", id), NAME_WIDTH), &mut tr.function, &choices.functions_in(&tr.function_schema));
    });
    ui.end_row();
    ui.label("Arguments:");
    ui.add(egui::TextEdit::singleline(&mut tr.arguments).desired_width(FIELD_WIDTH).hint_text("'audit', 3"));
    ui.end_row();
}

/// CONSTRAINT TRIGGER and what only applies to one. Such a trigger always fires AFTER,
/// once per row, so the grid is set to match.
fn constraint(ui: &mut Ui, id: u64, choices: &IndexChoices, tr: &mut TriggerDef) {
    ui.label("Constraint:");
    ui.checkbox(&mut tr.constraint, "");
    ui.end_row();
    if !tr.constraint {
        ui.disable();
    }
    deferrable_rows(ui, ("dtrigger", id), &mut tr.deferrable, &mut tr.deferred);
    ui.label("Referenced Table:");
    ui.horizontal(|ui| {
        search_combo(ui, (("dtrigger_ref_schema", id), SCHEMA_WIDTH), &mut tr.ref_schema, &choices.table_schemas());
        search_combo(ui, (("dtrigger_ref_table", id), NAME_WIDTH), &mut tr.ref_table, &choices.tables_in(&tr.ref_schema));
    });
    ui.end_row();
    if tr.constraint {
        tr.fires = "AFTER".into();
        tr.for_each = "ROW".into();
    }
}
