//! Read-only result grid used by the query editor and the Users list.

use eframe::egui::{self, pos2, vec2, RichText, Sense, Ui};
use egui_extras::{Column, TableBuilder};

use super::grid::{arrow, cell_lines, cell_text, col_width, header_cell, Arrow};
use super::menu::context_menu;
use crate::domain::{Cell, Row};
use crate::ui::theme::pal;

pub fn result_grid(ui: &mut Ui, id: impl std::hash::Hash + std::fmt::Debug, columns: &[String], rows: &[Row], sel: &mut Option<(usize, usize)>) {
    if columns.is_empty() {
        ui.add_space(8.0);
        ui.label(RichText::new("  No result set").color(pal().muted));
        return;
    }
    ui.push_id(id, |ui| table(ui, columns, rows, sel));
    copy_shortcut(ui, rows, *sel);
}

fn table(ui: &mut Ui, columns: &[String], rows: &[Row], sel: &mut Option<(usize, usize)>) {
    ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
    let mut tb = TableBuilder::new(ui)
        .striped(true)
        .sense(Sense::click())
        .resizable(true)
        .auto_shrink(false)
        .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
        .column(Column::exact(36.0));
    for (i, c) in columns.iter().enumerate() {
        tb = tb.column(Column::initial(col_width(c, rows, i)).at_least(40.0).clip(true));
    }
    tb.header(24.0, |mut h| {
        h.col(|ui| {
            header_cell(ui);
        });
        for c in columns {
            h.col(|ui| {
                header_cell(ui);
                ui.add_space(5.0);
                ui.add(egui::Label::new(RichText::new(c).strong()).truncate().selectable(false));
            });
        }
    })
    .body(|body| {
        body.rows(22.0, rows.len(), |mut row| {
            let r = row.index();
            row.col(|ui| {
                let rect = header_cell(ui);
                if matches!(sel, Some((sr, _)) if *sr == r) {
                    arrow(ui.painter(), pos2(rect.left() + 13.0, rect.center().y), 4.0, Arrow::Right, pal().accent);
                }
            });
            for (c, column) in columns.iter().enumerate() {
                let selected = *sel == Some((r, c));
                let (_, resp) = row.col(|ui| value_cell(ui, rows[r].get(c).unwrap_or(&None), selected));
                if resp.clicked() || resp.secondary_clicked() {
                    *sel = Some((r, c));
                }
                context_menu(&resp, |ui| copy_menu(ui, &rows[r], c, column));
            }
        });
    });
}

fn value_cell(ui: &mut Ui, v: &Cell, selected: bool) {
    let rect = ui.max_rect();
    if selected {
        ui.painter().rect_filled(rect, 0.0, pal().sel_bg);
    }
    cell_lines(ui, rect);
    cell_text(ui, v, pal().text);
}

fn copy_menu(ui: &mut Ui, row: &Row, c: usize, column: &str) {
    let copy = |ui: &mut Ui, label: &str, text: String| {
        if ui.button(label).clicked() {
            ui.ctx().copy_text(text);
            ui.close();
        }
    };
    copy(ui, "Copy", row.get(c).cloned().flatten().unwrap_or_default());
    copy(ui, "Copy Row (tab separated)", row.iter().map(|v| v.clone().unwrap_or_default()).collect::<Vec<_>>().join("\t"));
    copy(ui, "Copy Column Name", column.to_string());
}

/// Ctrl+C copies the selected cell when no text field has focus.
fn copy_shortcut(ui: &Ui, rows: &[Row], sel: Option<(usize, usize)>) {
    let Some((r, c)) = sel else { return };
    let pressed = ui.input(|i| i.modifiers.command && i.key_pressed(egui::Key::C)) && ui.ctx().memory(|m| m.focused().is_none());
    if let (true, Some(v)) = (pressed, rows.get(r).and_then(|row| row.get(c))) {
        ui.ctx().copy_text(v.clone().unwrap_or_default());
    }
}
