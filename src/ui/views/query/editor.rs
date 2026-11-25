//! The SQL editor with line numbers.

use eframe::egui::{self, FontId, RichText, Ui};

use super::QueryTab;
use crate::ui::theme::pal;
use crate::ui::widgets::sql_layouter;

const MIN_LINES: usize = 30;
const FONT_SIZE: f32 = 13.5;

pub(super) fn panel(ui: &mut Ui, t: &mut QueryTab) {
    egui::CentralPanel::default().frame(egui::Frame::new().fill(pal().surface)).show(ui, |ui| {
        egui::ScrollArea::both().id_salt(("qed", t.id)).auto_shrink(false).show(ui, |ui| {
            ui.horizontal_top(|ui| {
                line_numbers(ui, t.sql.matches('\n').count() + 1);
                editor(ui, t);
            });
        });
    });
}

fn line_numbers(ui: &mut Ui, lines: usize) {
    let nums: String = (1..=lines.max(MIN_LINES)).map(|n| n.to_string()).collect::<Vec<_>>().join("\n");
    egui::Frame::new().fill(pal().header_bg).inner_margin(egui::Margin { left: 6, right: 6, top: 2, bottom: 0 }).show(ui, |ui| {
        ui.add(egui::Label::new(RichText::new(nums).font(FontId::monospace(FONT_SIZE)).color(pal().muted)).selectable(false));
    });
}

fn editor(ui: &mut Ui, t: &mut QueryTab) {
    let mut layouter = sql_layouter;
    let out = egui::TextEdit::multiline(&mut t.sql)
        .id_salt(("sql", t.id))
        .font(FontId::monospace(FONT_SIZE))
        .code_editor()
        .frame(egui::Frame::NONE)
        .margin(egui::Margin { left: 6, right: 4, top: 2, bottom: 2 })
        .desired_width(f32::INFINITY)
        .desired_rows(MIN_LINES)
        .lock_focus(true)
        .layouter(&mut layouter)
        .show(ui);
    if out.response.response.changed() {
        t.dirty = true;
    }
    if let Some(cr) = out.cursor_range {
        let r = cr.as_sorted_char_range();
        t.selection = Some((r.start.0, r.end.0));
    }
}
