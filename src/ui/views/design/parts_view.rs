//! The designer's Indexes, Foreign Keys, Uniques, Checks, Excludes, Rules, Triggers and
//! Options tabs.

use eframe::egui::{self, Ui};

use super::cell_window::anchor_at;
use super::fk_view::{self, FkLists};
use super::form_rows::{body_panel, PanelText};
use super::index_cols_state::ColsTarget;
use super::index_cols_view;
use super::index_props_view;
use super::parts_grid::{grid, Cell, GridSpec};
use super::parts_state::PartKind;
use super::tick_state::TickTarget;
use super::tick_view;
use super::trigger_view;
use super::unique_view;
use super::DesignTab;
use crate::domain::{CheckDef, ExcludeDef, IndexDef, RuleDef, TableOptions};
use crate::sql::{DesignerCaps, OptionKind, OptionSpec, RULE_EVENTS};
use crate::ui::theme::pal;

pub(super) fn list(ui: &mut Ui, t: &mut DesignTab, k: PartKind) {
    if t.parts.loading {
        ui.horizontal(|ui| {
            ui.add_space(10.0);
            ui.spinner();
            ui.label("Loading…");
        });
        return;
    }
    match k {
        PartKind::Index => index_props_view::panel(ui, t),
        PartKind::Trigger => trigger_view::panel(ui, t),
        PartKind::Rule => rule_panel(ui, t),
        PartKind::ForeignKey if t.dialect().designer().constraint_options => fk_view::options_panel(ui, t),
        PartKind::Unique if t.dialect().designer().constraint_options => unique_view::options_panel(ui, t),
        _ => {}
    }
    let caps = t.dialect().designer();
    egui::CentralPanel::default().frame(egui::Frame::new().fill(pal().surface)).show(ui, |ui| grid_for(ui, t, k, caps));
    match k {
        PartKind::Index | PartKind::Exclude => index_cols_view::show(ui, t),
        PartKind::Unique | PartKind::Trigger => tick_view::show_ticks(ui, t),
        _ => {}
    }
}

fn grid_for(ui: &mut Ui, t: &mut DesignTab, k: PartKind, caps: &'static DesignerCaps) {
    let id = (k.tab_label(), t.id);
    if k == PartKind::ForeignKey {
        fk_view::want_ref_columns(t);
    }
    let fields = t.field_refs();
    let fk_lists = FkLists { fields: &fields, choices: &t.index_choices, refs: &t.ref_columns, comments: caps.constraint_comments };
    let editor = &mut t.parts;
    // A copy, so the grid can borrow the selection and the list at the same time.
    let mut selected = editor.sel(k);
    let sel = &mut selected;
    let p = &mut editor.parts;
    let comments = caps.constraint_comments;
    match k {
        PartKind::Index => {
            let columns = [("Name", 200.0), ("Fields", 280.0), ("Index method", 110.0), ("Unique", 64.0), ("Concurrently", 90.0), ("Comment", 160.0)];
            let opened = grid(ui, &GridSpec { id, columns: &columns }, &mut p.indexes, sel, |i: &mut IndexDef| index_cells(i, caps));
            if let Some((i, cell)) = opened {
                editor.open_cols(ColsTarget::Index(i));
                anchor_at(ui, index_cols_view::window_key(id.1), cell);
            }
        }
        PartKind::ForeignKey => fk_view::fk_grid(ui, (id, sel), &mut p.foreign_keys, &fk_lists),
        PartKind::Unique => {
            if let Some((i, cell)) = unique_view::unique_grid(ui, (id, sel), &mut p.uniques, comments) {
                editor.open_ticks(TickTarget::Unique(i), &fields);
                anchor_at(ui, tick_view::window_key(id.1), cell);
            }
        }
        PartKind::Check => {
            let spec = GridSpec { id, columns: &[("Name", 200.0), ("Expression", 360.0), ("No Inherit", 80.0), ("Comment", 160.0)] };
            grid(ui, &spec, &mut p.checks, sel, |c: &mut CheckDef| check_cells(c, caps));
        }
        PartKind::Exclude => {
            let spec = GridSpec { id, columns: &[("Name", 180.0), ("Index method", 110.0), ("Element", 280.0), ("Where", 200.0), ("Comment", 160.0)] };
            let opened = grid(ui, &spec, &mut p.excludes, sel, |e: &mut ExcludeDef| exclude_cells(e, caps));
            if let Some((i, cell)) = opened {
                editor.open_cols(ColsTarget::Exclude(i));
                anchor_at(ui, index_cols_view::window_key(id.1), cell);
            }
        }
        PartKind::Rule => {
            let spec = GridSpec { id, columns: &[("Name", 180.0), ("Event", 100.0), ("Do Instead", 80.0), ("Where", 240.0), ("Enable", 60.0), ("Comment", 160.0)] };
            grid(ui, &spec, &mut p.rules, sel, rule_cells);
        }
        PartKind::Trigger => {
            if let Some((i, cell)) = trigger_view::trigger_grid(ui, (id, sel), &mut p.triggers, caps) {
                editor.open_ticks(TickTarget::UpdateOf(i), &fields);
                anchor_at(ui, tick_view::window_key(id.1), cell);
            }
        }
    }
    *editor.sel_mut(k) = selected;
}

fn text_or_off(s: &mut String, on: bool) -> Cell<'_> {
    if on {
        Cell::Text(s)
    } else {
        Cell::Off
    }
}

fn check_or_off(b: &mut bool, on: bool) -> Cell<'_> {
    if on {
        Cell::Check(b)
    } else {
        Cell::Off
    }
}

fn index_cells<'b>(i: &'b mut IndexDef, caps: &DesignerCaps) -> Vec<Cell<'b>> {
    let concurrently = check_or_off(&mut i.concurrently, caps.index_concurrently);
    vec![Cell::Text(&mut i.name), Cell::Editor(&i.columns), Cell::Choice(&mut i.method, caps.index_methods), Cell::Check(&mut i.unique), concurrently, Cell::Text(&mut i.comment)]
}

fn exclude_cells<'b>(e: &'b mut ExcludeDef, caps: &DesignerCaps) -> Vec<Cell<'b>> {
    vec![Cell::Text(&mut e.name), Cell::Choice(&mut e.method, caps.index_methods), Cell::Editor(&e.elements), Cell::Text(&mut e.predicate), Cell::Text(&mut e.comment)]
}

fn rule_cells(r: &mut RuleDef) -> Vec<Cell<'_>> {
    vec![Cell::Text(&mut r.name), Cell::Choice(&mut r.event, &RULE_EVENTS), Cell::Check(&mut r.instead), Cell::Text(&mut r.condition), Cell::Check(&mut r.enabled), Cell::Text(&mut r.comment)]
}

fn check_cells<'b>(c: &'b mut CheckDef, caps: &DesignerCaps) -> Vec<Cell<'b>> {
    vec![Cell::Text(&mut c.name), Cell::Text(&mut c.expr), check_or_off(&mut c.no_inherit, caps.check_no_inherit), text_or_off(&mut c.comment, caps.constraint_comments)]
}

/// The selected rule's commands (after DO).
fn rule_panel(ui: &mut Ui, t: &mut DesignTab) {
    let sel = t.parts.sel(PartKind::Rule);
    let body = sel.and_then(|i| t.parts.parts.rules.get_mut(i)).map(|r| &mut r.body);
    let text = PanelText { label: "Definition", hint: "UPDATE t SET deleted = true WHERE id = old.id   (empty = NOTHING)", empty: "Select a rule to edit what it does." };
    body_panel(ui, ("drule", t.id), &text, body);
}

/// Storage options as the dialect lists them.
pub(super) fn options(ui: &mut Ui, t: &mut DesignTab) {
    if t.parts.loading {
        ui.spinner();
        return;
    }
    let caps = t.dialect().designer();
    let opts = &mut t.parts.parts.options;
    let id = t.id;
    egui::ScrollArea::both().id_salt(("dopts_s", id)).auto_shrink(false).show(ui, |ui| {
        egui::Frame::new().inner_margin(12).show(ui, |ui| option_grid(ui, id, caps.options, opts));
    });
}

fn option_grid(ui: &mut Ui, id: u64, specs: &[OptionSpec], opts: &mut TableOptions) {
    egui::Grid::new(("dopts", id)).num_columns(2).spacing([24.0, 8.0]).show(ui, |ui| {
        for spec in specs {
            option_row(ui, spec, opts);
            ui.end_row();
        }
    });
}

fn option_row(ui: &mut Ui, spec: &OptionSpec, opts: &mut TableOptions) {
    ui.label(format!("{}:", spec.label));
    let value = opts.entry(spec.key.to_string()).or_default();
    match spec.kind {
        OptionKind::Text => {
            ui.add(egui::TextEdit::singleline(value).desired_width(300.0));
        }
        OptionKind::Bool => {
            let mut on = value.as_str() == "true";
            if ui.checkbox(&mut on, "").changed() {
                *value = on.to_string();
            }
        }
    }
}
