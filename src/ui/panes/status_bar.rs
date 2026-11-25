//! The status bar: last message on the left, selection on the right.

use eframe::egui::{self, RichText, Ui};

use crate::app::{ConnStatus, Workspace};
use crate::ui::theme::pal;

pub fn status_bar(ui: &mut Ui, ws: &Workspace, status: &str) {
    ui.horizontal_centered(|ui| {
        ui.add_space(4.0);
        if ws.conns.iter().any(|c| c.status == ConnStatus::Opening) {
            ui.spinner();
        }
        ui.add(egui::Label::new(RichText::new(status).size(12.0)).truncate());
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.add_space(6.0);
            if let Some(text) = selection_text(ws) {
                ui.label(RichText::new(text).size(12.0).color(pal().muted));
            }
        });
    });
}

/// "Connection | db | N table(s) | object"
fn selection_text(ws: &Workspace) -> Option<String> {
    let c = ws.sel.cid.and_then(|c| ws.conn(c))?;
    let mut parts = vec![c.cfg.name.clone()];
    if let Some(db) = &ws.sel.db {
        parts.push(db.clone());
        if let Some(d) = c.db(db).filter(|d| d.loaded) {
            parts.push(format!("{} table(s)", d.tables.iter().filter(|t| !t.is_view).count()));
        }
    }
    parts.extend(ws.sel.obj.clone());
    Some(parts.join("    |    "))
}
