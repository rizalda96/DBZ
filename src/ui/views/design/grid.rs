//! The designer's fields grid.

use eframe::egui::{self, pos2, vec2, Rect, RichText, Sense, Ui};
use egui_extras::{Column, TableBuilder, TableRow};

use super::state::set_type;
use super::DesignTab;
use crate::domain::FieldDef;
use crate::ui::icons::{self, Icon};
use crate::ui::theme::pal;
use crate::ui::widgets::{arrow, cell_lines, header_cell, menu_config, Arrow};

const HEADERS: [&str; 8] = ["", "Name", "Type", "Length", "Decimals", "Not Null", "Key", "Comment"];

pub(super) fn show(ui: &mut Ui, t: &mut DesignTab) {
    let types = t.dialect().field_types();
    let n = t.fields.len();
    let mut select: Option<usize> = None;
    ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
    TableBuilder::new(ui)
        .id_salt(("dsgrid", t.id))
        .striped(true)
        .resizable(true)
        .auto_shrink(false)
        .sense(Sense::click())
        .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
        .column(Column::exact(28.0))
        .column(Column::initial(180.0).at_least(80.0).clip(true))
        .column(Column::initial(170.0).at_least(80.0).clip(true))
        .column(Column::initial(80.0).clip(true))
        .column(Column::initial(80.0).clip(true))
        .column(Column::initial(76.0).clip(true))
        .column(Column::initial(56.0).clip(true))
        .column(Column::remainder().at_least(120.0))
        .header(24.0, |mut h| {
            for l in HEADERS {
                h.col(|ui| {
                    header_cell(ui);
                    ui.add_space(5.0);
                    ui.label(RichText::new(l).strong());
                });
            }
        })
        .body(|body| {
            body.rows(26.0, n, |mut row| {
                let i = row.index();
                if field_row(&mut row, t, i, types) {
                    select = Some(i);
                }
            });
        });
    if let Some(i) = select {
        t.sel = Some(i);
    }
}

/// Draws one field; true when the user touched it (so it becomes selected).
fn field_row(row: &mut TableRow, t: &mut DesignTab, i: usize, types: &[&str]) -> bool {
    let selected = t.sel == Some(i);
    row.set_selected(selected);
    let (_, marker) = row.col(|ui| {
        let rect = header_cell(ui);
        if selected {
            arrow(ui.painter(), pos2(rect.left() + 13.0, rect.center().y), 4.0, Arrow::Right, pal().accent);
        }
    });
    let f = &mut t.fields[i];
    let mut touched = marker.clicked();
    touched |= text_cell(row, &mut f.name, ("fname", i));
    row.col(|ui| {
        cell_lines(ui, ui.max_rect());
        ui.add_space(2.0);
        touched |= ui.add(egui::TextEdit::singleline(&mut f.ty).desired_width(ui.available_width() - 40.0).id_salt(("ftype", i))).gained_focus();
        type_menu(ui, f, types);
    });
    touched |= text_cell(row, &mut f.length, ("flen", i));
    touched |= text_cell(row, &mut f.decimals, ("fdec", i));
    row.col(|ui| {
        cell_lines(ui, ui.max_rect());
        ui.add_space(24.0);
        touched |= ui.checkbox(&mut f.not_null, "").clicked();
    });
    touched |= key_cell(row, f);
    touched |= text_cell(row, &mut f.comment, ("fcom", i));
    touched
}

fn text_cell(row: &mut TableRow, text: &mut String, id: (&str, usize)) -> bool {
    let mut focused = false;
    row.col(|ui| {
        cell_lines(ui, ui.max_rect());
        ui.add_space(2.0);
        focused = ui.add(egui::TextEdit::singleline(text).desired_width(ui.available_width() - 6.0).id_salt(id)).gained_focus();
    });
    focused
}

/// The "…" button listing the dialect's types.
fn type_menu(ui: &mut Ui, f: &mut FieldDef, types: &[&str]) {
    egui::containers::menu::MenuButton::new("…").config(menu_config()).ui(ui, |ui| {
        egui::ScrollArea::vertical().max_height(300.0).show(ui, |ui| {
            for ty in types {
                if ui.button(*ty).clicked() {
                    set_type(f, ty);
                    ui.close();
                }
            }
        });
    });
}

/// Key icon cell; clicking toggles the primary key.
fn key_cell(row: &mut TableRow, f: &mut FieldDef) -> bool {
    let (_, rk) = row.col(|ui| {
        let rect = ui.max_rect();
        cell_lines(ui, rect);
        if f.key {
            icons::paint(ui.painter(), Rect::from_center_size(rect.center(), vec2(16.0, 16.0)), Icon::Key);
        }
    });
    if !rk.clicked() {
        return false;
    }
    f.key = !f.key;
    if f.key {
        f.not_null = true;
    }
    true
}
