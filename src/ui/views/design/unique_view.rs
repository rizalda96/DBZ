//! The designer's Uniques tab: the grid (fields picked in a tick list that opens from
//! the cell's "…"), and the tablespace / fill factor / DEFERRABLE panel under it on
//! servers that have those.

use eframe::egui::{self, Rect, RichText, Stroke, Ui};

use super::form_rows::{combo, deferrable_rows, FIELD_WIDTH};
use super::parts_grid::{grid, Cell, GridSpec};
use super::parts_state::PartKind;
use super::DesignTab;
use crate::domain::UniqueDef;
use crate::ui::theme::pal;

const COLUMNS: [(&str, f32); 3] = [("Name", 220.0), ("Fields", 320.0), ("Comment", 200.0)];

/// Returns the row whose Fields "…" was clicked, with the cell's rect.
pub(super) fn unique_grid(ui: &mut Ui, (id, sel): ((&'static str, u64), &mut Option<usize>), uniques: &mut [UniqueDef], comments: bool) -> Option<(usize, Rect)> {
    grid(ui, &GridSpec { id, columns: &COLUMNS }, uniques, sel, |u: &mut UniqueDef| {
        let comment = if comments { Cell::Text(&mut u.comment) } else { Cell::Off };
        vec![Cell::Text(&mut u.name), Cell::Editor(&u.columns), comment]
    })
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
