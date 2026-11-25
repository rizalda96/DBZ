//! The read-only "SQL Preview" page of a designer: statements, highlighted.

use eframe::egui::{self, FontId, Ui};

use super::highlight::sql_layouter;

/// Each statement ended with `;`, a blank line between them; `empty` when there are none.
pub fn sql_preview(ui: &mut Ui, statements: &[String], empty: &str) {
    let mut text = statements.iter().map(|s| format!("{};", s)).collect::<Vec<_>>().join("\n\n");
    if text.is_empty() {
        text = empty.to_string();
    }
    let mut layouter = sql_layouter;
    egui::ScrollArea::both().auto_shrink(false).show(ui, |ui| {
        let view = egui::TextEdit::multiline(&mut text).font(FontId::monospace(13.0)).desired_width(f32::INFINITY).interactive(true).frame(egui::Frame::NONE);
        ui.add(view.layouter(&mut layouter));
    });
}
