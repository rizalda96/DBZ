//! The designer's Uniques tab: the grid (fields picked in a tick list that opens from
//! the cell's "…"), and the tablespace / fill factor / DEFERRABLE panel under it on
//! servers that have those.

use eframe::egui::{self, vec2, Rect, RichText, Stroke, Ui};

use super::cell_window::{cell_window, WindowKey};
use super::form_rows::{combo, deferrable_rows, FIELD_WIDTH};
use super::parts_grid::{grid, Cell, GridSpec};
use super::parts_state::PartKind;
use super::tick_state::{TickEditor, TickRow};
use super::DesignTab;
use crate::domain::UniqueDef;
use crate::ui::icons::Icon;
use crate::ui::theme::pal;
use crate::ui::widgets::{buttons, icon_btn, Choice};

const COLUMNS: [(&str, f32); 3] = [("Name", 220.0), ("Fields", 320.0), ("Comment", 200.0)];
const TICK_COLUMNS: [(&str, f32); 2] = [("", 30.0), ("Name", 220.0)];
const TICK_SIZE: [f32; 2] = [340.0, 220.0];

/// The window's key for [`cell_window`] and [`super::cell_window::anchor_at`].
pub(super) fn window_key(tab: u64) -> WindowKey {
    ("unique_fields", tab)
}

/// Returns the row whose Fields "…" was clicked, with the cell's rect.
pub(super) fn unique_grid(ui: &mut Ui, (id, sel): ((&'static str, u64), &mut Option<usize>), uniques: &mut [UniqueDef], comments: bool) -> Option<(usize, Rect)> {
    grid(ui, &GridSpec { id, columns: &COLUMNS }, uniques, sel, |u: &mut UniqueDef| {
        let comment = if comments { Cell::Text(&mut u.comment) } else { Cell::Off };
        vec![Cell::Text(&mut u.name), Cell::Editor(&u.columns), comment]
    })
}

/// The unique fields editor while it is open: tick the key's fields, order them with
/// the arrows.
pub(super) fn show_ticks(ui: &Ui, t: &mut DesignTab) {
    let id = t.id;
    let Some(ed) = t.parts.ticks.as_mut() else { return };
    let choice = cell_window(ui, window_key(id), "Unique Fields", TICK_SIZE, |ui| tick_body(ui, id, ed));
    if !choice.keep() {
        t.parts.close_ticks(choice == Choice::Ok);
    }
}

fn tick_body(ui: &mut Ui, id: u64, ed: &mut TickEditor) -> Choice {
    let grid_size = vec2(ui.available_width(), ui.available_height() - 40.0);
    ui.allocate_ui(grid_size, |ui| {
        let spec = GridSpec { id: ("unique_ticks", id), columns: &TICK_COLUMNS };
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

/// Tablespace, fill factor and DEFERRABLE of the selected unique constraint.
pub(super) fn options_panel(ui: &mut Ui, t: &mut DesignTab) {
    let id = t.id;
    let tablespaces = &t.index_choices.tablespaces;
    let sel = t.parts.sel(PartKind::Unique);
    let unique = sel.and_then(|i| t.parts.parts.uniques.get_mut(i));
    let frame = egui::Frame::new().fill(pal().toolbar_bg).inner_margin(10).stroke(Stroke::new(1.0, pal().border));
    egui::Panel::bottom(("dunique", id)).resizable(true).default_size(150.0).frame(frame).show(ui, |ui| {
        let Some(u) = unique else {
            ui.label(RichText::new("Select a unique constraint to see its options.").color(pal().muted));
            return;
        };
        egui::Grid::new(("dunique_opts", id)).num_columns(2).spacing([24.0, 8.0]).min_col_width(140.0).show(ui, |ui| unique_options(ui, id, tablespaces, u));
    });
}

fn unique_options(ui: &mut Ui, id: u64, tablespaces: &[String], u: &mut UniqueDef) {
    ui.label("Tablespace:");
    combo(ui, ("dunique_ts", id), &mut u.tablespace, tablespaces);
    ui.end_row();
    ui.label("Fill Factor (%):");
    ui.add(egui::TextEdit::singleline(&mut u.fill_factor).desired_width(FIELD_WIDTH).hint_text("10 – 100"));
    ui.end_row();
    deferrable_rows(ui, ("dunique", id), &mut u.deferrable, &mut u.deferred);
}
