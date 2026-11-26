//! A SQL text box with line numbers and syntax colours: the query editor and the
//! designer's trigger definitions. The caller puts it in a scroll area.

use eframe::egui::{self, FontId, RichText, Ui};

use super::sql_layouter;
use crate::ui::theme::pal;

const FONT_SIZE: f32 = 13.5;

/// Draws `text` with at least `min_lines` numbered lines.
pub fn code_editor(ui: &mut Ui, salt: impl std::hash::Hash + std::fmt::Debug, text: &mut String, min_lines: usize) -> egui::text_edit::TextEditOutput {
    ui.horizontal_top(|ui| {
        line_numbers(ui, text.matches('\n').count() + 1, min_lines);
        let mut layouter = sql_layouter;
        egui::TextEdit::multiline(text)
            .id_salt(salt)
            .font(FontId::monospace(FONT_SIZE))
            .code_editor()
            .frame(egui::Frame::NONE)
            .margin(egui::Margin { left: 6, right: 4, top: 2, bottom: 2 })
            .desired_width(f32::INFINITY)
            .desired_rows(min_lines)
            .lock_focus(true)
            .layouter(&mut layouter)
            .show(ui)
    })
    .inner
}

fn line_numbers(ui: &mut Ui, lines: usize, min_lines: usize) {
    let nums: String = (1..=lines.max(min_lines)).map(|n| n.to_string()).collect::<Vec<_>>().join("\n");
    egui::Frame::new().fill(pal().header_bg).inner_margin(egui::Margin { left: 6, right: 6, top: 2, bottom: 0 }).show(ui, |ui| {
        ui.add(egui::Label::new(RichText::new(nums).font(FontId::monospace(FONT_SIZE)).color(pal().muted)).selectable(false));
    });
}
