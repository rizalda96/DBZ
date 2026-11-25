//! The designer's fields grid.

use eframe::egui::{self, pos2, vec2, Rect, RichText, Sense, Ui};
use egui_extras::{Column, TableBuilder, TableRow};

use super::state::{set_type, FieldCol};
use super::DesignTab;
use crate::domain::FieldDef;
use crate::ui::icons::{self, Icon};
use crate::ui::theme::pal;
use crate::ui::widgets::{arrow, cell_lines, header_cell, menu_config, Arrow};

const HEADERS: [&str; 8] = ["", "Name", "Type", "Length", "Decimals", "Not Null", "Key", "Comment"];

pub(super) fn show(ui: &mut Ui, t: &mut DesignTab) {
    let mut out = RowOut::default();
    // TableBuilder only scrolls vertically; many wide columns need an outer horizontal scroll.
    egui::ScrollArea::horizontal().id_salt(("dsgrid_h", t.id)).auto_shrink(false).show(ui, |ui| table(ui, t, &mut out));
    if let Some(i) = out.touched {
        t.sel = Some(i);
    }
    keyboard(ui, t, out.focused);
}

fn table(ui: &mut Ui, t: &mut DesignTab, out: &mut RowOut) {
    let types = t.dialect().field_types();
    let n = t.fields.len();
    ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
    let mut tb = TableBuilder::new(ui)
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
        .column(Column::remainder().at_least(120.0));
    if let Some((r, _)) = t.focus {
        tb = tb.scroll_to_row(r, None);
    }
    tb.header(24.0, |mut h| {
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
            out.merge(i, field_row(&mut row, t, i, types));
        });
    });
}

/// What the rows reported this frame.
#[derive(Default)]
struct RowOut {
    /// Row the user clicked or tabbed into.
    touched: Option<usize>,
    /// Cell whose text field has keyboard focus.
    focused: Option<(usize, FieldCol)>,
}

impl RowOut {
    fn merge(&mut self, i: usize, (touched, focused): (bool, Option<FieldCol>)) {
        if touched {
            self.touched = Some(i);
        }
        if let Some(col) = focused {
            self.focused = Some((i, col));
        }
    }
}

/// Arrow Up/Down move to the field above/below, keeping the column; Down on the last
/// field adds a new one. Only while a grid cell (or nothing at all) has focus.
fn keyboard(ui: &Ui, t: &mut DesignTab, focused: Option<(usize, FieldCol)>) {
    let nothing_focused = ui.ctx().memory(|m| m.focused().is_none());
    if focused.is_none() && !(nothing_focused && t.sel.is_some()) {
        return;
    }
    let (down, up) = ui.input(|i| (i.key_pressed(egui::Key::ArrowDown), i.key_pressed(egui::Key::ArrowUp)));
    if !down && !up {
        return;
    }
    if let Some((i, _)) = focused {
        t.sel = Some(i);
    }
    t.arrow_key(down, focused.map_or(FieldCol::Name, |f| f.1));
}

/// Draws one field; returns whether the user touched it (so it becomes selected)
/// and which of its text cells has focus.
fn field_row(row: &mut TableRow, t: &mut DesignTab, i: usize, types: &[&str]) -> (bool, Option<FieldCol>) {
    let selected = t.sel == Some(i);
    row.set_selected(selected);
    let (_, marker) = row.col(|ui| {
        let rect = header_cell(ui);
        if selected {
            arrow(ui.painter(), pos2(rect.left() + 13.0, rect.center().y), 4.0, Arrow::Right, pal().accent);
        }
    });
    let want = t.focus.filter(|(r, _)| *r == i).map(|(_, c)| c);
    if want.is_some() {
        t.focus = None;
    }
    let f = &mut t.fields[i];
    let mut cells = Cells { want, touched: marker.clicked(), focused: None };
    cells.text(row, &mut f.name, FieldCol::Name, i);
    row.col(|ui| {
        cell_lines(ui, ui.max_rect());
        ui.add_space(2.0);
        let te = ui.add(egui::TextEdit::singleline(&mut f.ty).desired_width(ui.available_width() - 40.0).id_salt(("ftype", i)));
        cells.track(&te, FieldCol::Type);
        type_menu(ui, f, types);
    });
    cells.text(row, &mut f.length, FieldCol::Length, i);
    cells.text(row, &mut f.decimals, FieldCol::Decimals, i);
    row.col(|ui| {
        cell_lines(ui, ui.max_rect());
        ui.add_space(24.0);
        cells.touched |= ui.checkbox(&mut f.not_null, "").clicked();
    });
    cells.touched |= key_cell(row, f);
    cells.text(row, &mut f.comment, FieldCol::Comment, i);
    (cells.touched, cells.focused)
}

/// Focus bookkeeping for the text cells of one row.
struct Cells {
    /// Column to focus now (set by the arrow keys last frame).
    want: Option<FieldCol>,
    touched: bool,
    focused: Option<FieldCol>,
}

impl Cells {
    fn text(&mut self, row: &mut TableRow, text: &mut String, col: FieldCol, i: usize) {
        row.col(|ui| {
            cell_lines(ui, ui.max_rect());
            ui.add_space(2.0);
            let te = ui.add(egui::TextEdit::singleline(text).desired_width(ui.available_width() - 6.0).id_salt((col, i)));
            self.track(&te, col);
        });
    }

    fn track(&mut self, te: &egui::Response, col: FieldCol) {
        if self.want == Some(col) {
            te.request_focus();
        }
        self.touched |= te.gained_focus();
        if te.has_focus() {
            self.focused = Some(col);
        }
    }
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
