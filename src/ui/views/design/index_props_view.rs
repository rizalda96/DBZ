//! The Indexes tab's properties panel under the grid: tablespace, storage parameters
//! and the WHERE predicate of the selected index, on servers that have them.

use eframe::egui::{self, RichText, Stroke, Ui};

use super::parts_state::PartKind;
use super::DesignTab;
use crate::domain::IndexDef;
use crate::sql::{DesignerCaps, IndexParamSpec};
use crate::ui::theme::pal;
use crate::ui::widgets::{buttons, win, Choice};

use super::form_rows::{combo, FIELD_WIDTH};

/// Draws the panel (and the predicate editor while it is open).
pub(super) fn panel(ui: &mut Ui, t: &mut DesignTab) {
    let caps = t.dialect().designer();
    if caps.index_params.is_empty() && !caps.index_placement {
        return;
    }
    let id = t.id;
    let frame = egui::Frame::new().fill(pal().toolbar_bg).inner_margin(10).stroke(Stroke::new(1.0, pal().border));
    let tablespaces = &t.index_choices.tablespaces;
    let index = t.parts.sel(PartKind::Index).and_then(|i| t.parts.parts.indexes.get_mut(i));
    egui::Panel::bottom(("dindex", id)).resizable(true).default_size(240.0).frame(frame).show(ui, |ui| {
        let Some(ix) = index else {
            ui.label(RichText::new("Select an index to see its properties.").color(pal().muted));
            return;
        };
        egui::Grid::new(("dindex_props", id)).num_columns(2).spacing([24.0, 8.0]).show(ui, |ui| props(ui, (id, caps), tablespaces, ix));
    });
    predicate_window(ui, t);
}

/// In Navicat's order: tablespace, the parameters, then the predicate.
fn props(ui: &mut Ui, (id, caps): (u64, &DesignerCaps), tablespaces: &[String], ix: &mut IndexDef) {
    if caps.index_placement {
        ui.label("Tablespace:");
        combo(ui, ("ix_ts", id), &mut ix.tablespace, tablespaces);
        ui.end_row();
    }
    for p in caps.index_params {
        param_row(ui, id, p, ix);
        ui.end_row();
    }
    if caps.index_placement {
        ui.label("Constraint:");
        ui.horizontal(|ui| {
            ui.add(egui::TextEdit::singleline(&mut ix.predicate).desired_width(FIELD_WIDTH - 32.0).hint_text("WHERE condition (partial index)"));
            if ui.button("…").on_hover_text("Edit in a larger box").clicked() {
                let backup = ix.predicate.clone();
                ui.data_mut(|d| d.insert_temp(predicate_key(id), backup));
            }
        });
        ui.end_row();
    }
}

/// A parameter the index method doesn't take is shown greyed out.
fn param_row(ui: &mut Ui, id: u64, p: &IndexParamSpec, ix: &mut IndexDef) {
    let on = p.applies(&ix.method);
    ui.add_enabled(on, egui::Label::new(format!("{}:", p.label)));
    let mut value = ix.storage.get(p.key).cloned().unwrap_or_default();
    let changed = ui
        .add_enabled_ui(on, |ui| {
            if p.values.is_empty() {
                return ui.add(egui::TextEdit::singleline(&mut value).desired_width(FIELD_WIDTH)).changed();
            }
            let options: Vec<String> = p.values.iter().map(|v| v.to_string()).collect();
            combo(ui, ("ix_param", id, p.key), &mut value, &options)
        })
        .inner;
    if changed {
        ix.storage.insert(p.key.to_string(), value);
    }
}



/// Egui memory key of the open predicate editor; holds the text it was opened with.
fn predicate_key(tab: u64) -> egui::Id {
    egui::Id::unique(("ix_predicate", tab))
}

/// A larger box for the selected index's WHERE predicate; Cancel restores it.
fn predicate_window(ui: &Ui, t: &mut DesignTab) {
    let key = predicate_key(t.id);
    let Some(backup) = ui.data(|d| d.get_temp::<String>(key)) else { return };
    let index = t.parts.sel(PartKind::Index).and_then(|i| t.parts.parts.indexes.get_mut(i));
    let Some(ix) = index else {
        ui.data_mut(|d| d.remove::<String>(key));
        return;
    };
    let mut choice = Choice::None;
    win("Constraint").id(egui::Id::unique(("ix_predicate_win", t.id))).show(ui.ctx(), |ui| {
        ui.label("Rows the index covers (WHERE …):");
        ui.add(egui::TextEdit::multiline(&mut ix.predicate).desired_width(460.0).desired_rows(6).font(egui::TextStyle::Monospace));
        choice = buttons(ui, "OK", true);
    });
    if choice.keep() {
        return;
    }
    if choice == Choice::Cancel {
        ix.predicate = backup;
    }
    ui.data_mut(|d| d.remove::<String>(key));
}
