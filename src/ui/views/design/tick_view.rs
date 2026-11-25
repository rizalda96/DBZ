//! The field tick list window that opens from a "…" cell: a unique constraint's
//! Fields or a trigger's Update of Fields. Tick the fields, order them with the arrows.

use eframe::egui::{vec2, Ui};

use super::cell_window::{cell_window, WindowKey};
use super::parts_grid::{grid, Cell, GridSpec};
use super::tick_state::{TickEditor, TickRow, TickTarget};
use super::DesignTab;
use crate::ui::icons::Icon;
use crate::ui::widgets::{buttons, icon_btn, Choice};

const COLUMNS: [(&str, f32); 2] = [("", 30.0), ("Name", 220.0)];
const SIZE: [f32; 2] = [340.0, 220.0];

/// The window's key for [`cell_window`] and [`super::cell_window::anchor_at`].
pub(super) fn window_key(tab: u64) -> WindowKey {
    ("field_ticks", tab)
}

/// Draws the window while it is open; OK writes the ticked fields back.
pub(super) fn show_ticks(ui: &Ui, t: &mut DesignTab) {
    let id = t.id;
    let Some(ed) = t.parts.ticks.as_mut() else { return };
    let title = match ed.target {
        TickTarget::Unique(_) => "Unique Fields",
        TickTarget::UpdateOf(_) => "Update of Fields",
    };
    let choice = cell_window(ui, window_key(id), title, SIZE, |ui| body(ui, id, ed));
    if !choice.keep() {
        t.parts.close_ticks(choice == Choice::Ok);
    }
}

fn body(ui: &mut Ui, id: u64, ed: &mut TickEditor) -> Choice {
    let grid_size = vec2(ui.available_width(), ui.available_height() - 40.0);
    ui.allocate_ui(grid_size, |ui| {
        let spec = GridSpec { id: ("field_ticks", id), columns: &COLUMNS };
        grid(ui, &spec, &mut ed.rows, &mut ed.sel, |r: &mut TickRow| vec![Cell::Check(&mut r.on), Cell::Label(&r.name)]);
    });
    let mut choice = Choice::None;
    ui.horizontal(|ui| {
        if icon_btn(ui, Icon::MoveUp, "Move Up", ed.can_move(-1)).clicked() {
            ed.move_sel(-1);
        }
        if icon_btn(ui, Icon::MoveDown, "Move Down", ed.can_move(1)).clicked() {
            ed.move_sel(1);
        }
        choice = buttons(ui, "OK", true);
    });
    choice
}
