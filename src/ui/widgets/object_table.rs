//! The list of databases / tables / files in the Objects tab.

use eframe::egui::{self, vec2, Response, RichText, Sense, Stroke, Ui};
use egui_extras::{Column, TableBuilder, TableRow};

use super::menu::context_menu;
use crate::ui::icons::{self, Icon};
use crate::ui::theme::pal;

pub enum RowEv<'a> {
    Click,
    Double,
    Menu(&'a mut Ui),
}

/// What one row shows.
pub struct ObjRow {
    pub cells: Vec<String>,
    pub icon: Icon,
    pub selected: bool,
}

pub struct ObjectTable<'a> {
    pub headers: &'a [&'a str],
    pub rows: usize,
}

impl ObjectTable<'_> {
    /// Draws the table; `row(i)` describes row i and `on` receives clicks and menus.
    pub fn show(self, ui: &mut Ui, id: impl std::hash::Hash + std::fmt::Debug, row: impl Fn(usize) -> ObjRow, on: impl FnMut(usize, RowEv)) {
        ui.push_id(id, |ui| self.table(ui, row, on));
    }

    fn table(&self, ui: &mut Ui, row: impl Fn(usize) -> ObjRow, mut on: impl FnMut(usize, RowEv)) {
        ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
        let mut tb = TableBuilder::new(ui).striped(false).sense(Sense::click()).resizable(true).auto_shrink(false).cell_layout(egui::Layout::left_to_right(egui::Align::Center));
        for i in 0..self.headers.len() {
            tb = tb.column(self.column(i));
        }
        tb.header(26.0, |mut h| {
            for l in self.headers {
                h.col(|ui| header(ui, l));
            }
        })
        .body(|body| {
            body.rows(24.0, self.rows, |mut tr| {
                let i = tr.index();
                if let Some(resp) = draw_row(&mut tr, &row(i)) {
                    row_events(i, &resp, &mut on);
                }
            });
        });
    }

    fn column(&self, i: usize) -> Column {
        if i == 0 {
            Column::initial(230.0).at_least(120.0).clip(true)
        } else if i + 1 == self.headers.len() {
            Column::remainder().at_least(80.0).clip(true)
        } else {
            Column::initial(120.0).at_least(50.0).clip(true)
        }
    }
}

fn row_events(i: usize, resp: &Response, on: &mut impl FnMut(usize, RowEv)) {
    if resp.double_clicked() {
        on(i, RowEv::Double);
    } else if resp.clicked() || resp.secondary_clicked() {
        on(i, RowEv::Click);
    }
    context_menu(resp, |ui| on(i, RowEv::Menu(ui)));
}

fn header(ui: &mut Ui, label: &str) {
    let r = ui.max_rect();
    ui.painter().line_segment([r.left_bottom(), r.right_bottom()], Stroke::new(1.0, pal().border));
    ui.painter().line_segment([r.right_top() + vec2(0.0, 5.0), r.right_bottom() - vec2(0.0, 5.0)], Stroke::new(1.0, pal().grid_line));
    ui.add_space(8.0);
    ui.label(RichText::new(label).color(pal().text_dim).size(12.5));
}

/// Draws the cells of one row; returns the response covering the whole row.
fn draw_row(tr: &mut TableRow, row: &ObjRow) -> Option<Response> {
    let mut all: Option<Response> = None;
    for (c, v) in row.cells.iter().enumerate() {
        let (_, resp) = tr.col(|ui| cell(ui, c, v, row));
        all = Some(match all {
            None => resp,
            Some(a) => a.union(resp),
        });
    }
    all
}

fn cell(ui: &mut Ui, c: usize, v: &str, row: &ObjRow) {
    if row.selected {
        ui.painter().rect_filled(ui.max_rect(), 0.0, pal().sel_bg);
    }
    ui.add_space(8.0);
    if c == 0 {
        let (ir, _) = ui.allocate_exact_size(vec2(16.0, 16.0), Sense::hover());
        icons::paint(ui.painter(), ir, row.icon);
        ui.add_space(6.0);
    }
    let color = if c == 0 { pal().text } else { pal().text_dim };
    ui.add(egui::Label::new(RichText::new(v).color(color)).truncate().selectable(false));
}
