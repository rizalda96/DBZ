//! The SQL editor with line numbers.

use eframe::egui::{self, Ui};

use super::QueryTab;
use crate::ui::theme::pal;
use crate::ui::widgets::code_editor;

const MIN_LINES: usize = 30;

pub(super) fn panel(ui: &mut Ui, t: &mut QueryTab) {
    egui::CentralPanel::default().frame(egui::Frame::new().fill(pal().surface)).show(ui, |ui| {
        egui::ScrollArea::both().id_salt(("qed", t.id)).auto_shrink(false).show(ui, |ui| editor(ui, t));
    });
}

fn editor(ui: &mut Ui, t: &mut QueryTab) {
    let out = code_editor(ui, ("sql", t.id), &mut t.sql, MIN_LINES);
    if out.response.response.changed() {
        t.dirty = true;
    }
    if let Some(cr) = out.cursor_range {
        let r = cr.as_sorted_char_range();
        t.selection = Some((r.start.0, r.end.0));
    }
}
