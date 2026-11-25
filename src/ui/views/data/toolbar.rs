//! Data tab toolbar and the WHERE filter row.

use eframe::egui::{self, FontId, RichText, Ui};

use super::{reload, DataTab};
use crate::app::{Act, ObjRef};
use crate::ui::icons::Icon;
use crate::ui::theme::pal;
use crate::ui::widgets::{tool_btn, tool_sep, toolbar_frame};

pub(super) fn show(ui: &mut Ui, t: &mut DataTab, editable: bool, acts: &mut Vec<Act>) {
    toolbar_frame().show(ui, |ui| {
        ui.horizontal(|ui| buttons(ui, t, editable, acts));
        if t.show_filter {
            ui.horizontal(|ui| filter_row(ui, t, acts));
        }
    });
}

fn buttons(ui: &mut Ui, t: &mut DataTab, editable: bool, acts: &mut Vec<Act>) {
    ui.spacing_mut().item_spacing.x = 2.0;
    if tool_btn(ui, Some(Icon::Filter), if t.show_filter { "Hide Filter" } else { "Filter" }, true).clicked() {
        t.show_filter = !t.show_filter;
    }
    if tool_btn(ui, None, "Clear Sort", t.order.is_some()).clicked() {
        t.order = None;
        reload(t, acts);
    }
    tool_sep(ui);
    if tool_btn(ui, Some(Icon::Import), "Import", editable).clicked() {
        acts.push(Act::Import(t.cid, t.db.clone(), Some(t.table.clone())));
    }
    if tool_btn(ui, Some(Icon::Export), "Export", true).clicked() {
        acts.push(Act::Export(ObjRef::new(t.cid, &t.db, &t.table)));
    }
    tool_sep(ui);
    if tool_btn(ui, Some(Icon::Design), "Design Table", editable).clicked() {
        acts.push(Act::DesignTable(t.cid, t.db.clone(), Some(t.table.clone())));
    }
    if t.loading || t.saving {
        ui.add_space(8.0);
        ui.spinner();
    }
}

fn filter_row(ui: &mut Ui, t: &mut DataTab, acts: &mut Vec<Act>) {
    ui.label(RichText::new("WHERE").strong().color(pal().accent));
    let edit = egui::TextEdit::singleline(&mut t.filter).hint_text("e.g. country = 'Japan' AND id > 10").desired_width(ui.available_width() - 150.0).font(FontId::monospace(13.0));
    let r = ui.add(edit);
    let enter = r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
    if ui.button("Apply").clicked() || enter {
        t.filter_applied = t.filter.clone();
        t.offset = 0;
        reload(t, acts);
    }
    if ui.button("Clear").clicked() {
        t.filter.clear();
        t.filter_applied.clear();
        t.offset = 0;
        reload(t, acts);
    }
}
