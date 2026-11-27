//! The result panel under the editor: Message tab and one tab per result set.

use eframe::egui::{self, RichText, Stroke, Ui};

use super::QueryTab;
use crate::ui::theme::pal;
use crate::ui::widgets::result_grid;

pub(super) fn panel(ui: &mut Ui, t: &mut QueryTab) {
    egui::Panel::bottom(("qres", t.id))
        .resizable(true)
        .default_size(280.0)
        .min_size(80.0)
        .frame(egui::Frame::new().fill(pal().surface).stroke(Stroke::new(1.0, pal().border)))
        .show(ui, |ui| {
            tabs_bar(ui, t);
            if t.view == 0 {
                message(ui, t);
            } else if let Some(rs) = t.results.get(t.view - 1) {
                let sorted = t.sorted();
                if let Some(col) = result_grid(ui, ("qgrid", t.id, t.view), rs, &mut t.sel_cell, sorted) {
                    t.sort_by(col);
                }
            }
        });
}

fn tabs_bar(ui: &mut Ui, t: &mut QueryTab) {
    egui::Frame::new().fill(pal().header_bg).inner_margin(egui::Margin::symmetric(4, 2)).show(ui, |ui| {
        ui.horizontal(|ui| {
            if ui.selectable_label(t.view == 0, "Message").clicked() {
                t.view = 0;
            }
            result_tabs(ui, t);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| timing(ui, t));
        });
    });
}

fn result_tabs(ui: &mut Ui, t: &mut QueryTab) {
    let with_rows = t.results.iter().enumerate().filter(|(_, r)| r.has_rows).map(|(i, _)| i).collect::<Vec<_>>();
    for (n, i) in with_rows.into_iter().enumerate() {
        if ui.selectable_label(t.view == i + 1, format!("Result {}", n + 1)).clicked() {
            t.view = i + 1;
            t.sel_cell = None;
        }
    }
}

fn timing(ui: &mut Ui, t: &QueryTab) {
    let Some(e) = t.elapsed else { return };
    let records = if t.view > 0 { format!("   Records: {}", t.shown_rows()) } else { String::new() };
    ui.label(RichText::new(format!("Query time: {:.3}s{}", e.as_secs_f64(), records)).color(pal().muted));
}

fn message(ui: &mut Ui, t: &QueryTab) {
    let msg = t.message_text();
    let color = if t.error.is_some() { pal().err } else { pal().text };
    egui::ScrollArea::both().auto_shrink(false).show(ui, |ui| {
        ui.add_space(4.0);
        ui.add(egui::Label::new(RichText::new(msg).monospace().color(color)).selectable(true));
    });
}
