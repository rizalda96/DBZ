//! The small editor windows that open from a grid cell's "…" (index fields, unique
//! fields): placed under that cell when they open, then movable by their title bar.

use eframe::egui::{self, Pos2, Rect, Ui};

use crate::ui::widgets::Choice;

/// Which window, in which designer tab.
pub(super) type WindowKey = (&'static str, u64);

fn anchor_id(key: WindowKey) -> egui::Id {
    egui::Id::unique(("cell_window_at", key))
}

/// The window opens under `cell` (once; it stays where the user drags it after).
pub(super) fn anchor_at(ui: &Ui, key: WindowKey, cell: Rect) {
    ui.data_mut(|d| d.insert_temp(anchor_id(key), cell.left_bottom()));
}

/// Draws the window; `body` reports how the user left it.
pub(super) fn cell_window(ui: &Ui, key: WindowKey, title: &str, size: [f32; 2], body: impl FnOnce(&mut Ui) -> Choice) -> Choice {
    let mut window = egui::Window::new(title).id(egui::Id::unique(("cell_window", key))).collapsible(false).resizable(false).movable(true).fixed_size(size);
    if let Some(at) = ui.data_mut(|d| d.remove_temp::<Pos2>(anchor_id(key))) {
        window = window.current_pos(at);
    }
    window.show(ui.ctx(), body).and_then(|r| r.inner).unwrap_or(Choice::None)
}
