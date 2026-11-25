//! The editable data grid.

use eframe::egui::{self, pos2, vec2, Rect, RichText, Sense, Ui};
use egui_extras::{Column, TableBuilder, TableRow};

use super::cell_menu::{self, CellCmd};
use super::{reload, DataTab};
use crate::app::Act;
use crate::domain::Row;
use crate::ui::icons::{self, Icon};
use crate::ui::theme::{pal, CHANGED_DOT};
use crate::ui::widgets::{arrow, cell_lines, cell_text, col_width, context_menu, header_cell, Arrow};

/// What the user did in the grid this frame, applied after drawing.
#[derive(Default)]
struct Events {
    header_click: Option<usize>,
    start_edit: Option<(usize, usize)>,
    cmd: Option<(usize, usize, CellCmd)>,
}

pub(super) fn show(ui: &mut Ui, t: &mut DataTab, editable: bool, acts: &mut Vec<Act>) {
    keyboard(ui, t, editable);
    let keys: Vec<bool> = t.columns.iter().map(|c| t.fields.iter().any(|f| &f.name == c && f.key)).collect();
    let preview: Vec<Row> = t.rows.iter().take(50).map(|r| r.cur.clone()).collect();
    let mut ev = Events::default();
    ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
    let mut tb = TableBuilder::new(ui)
        .id_salt(("dgrid", t.id))
        .striped(true)
        .sense(Sense::click())
        .resizable(true)
        .auto_shrink(false)
        .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
        .column(Column::exact(36.0));
    for (i, c) in t.columns.iter().enumerate() {
        let key_room = if keys[i] { 16.0 } else { 0.0 };
        tb = tb.column(Column::initial(col_width(c, &preview, i) + key_room).at_least(40.0).clip(true));
    }
    if let Some(r) = t.scroll_to.take() {
        tb = tb.scroll_to_row(r, None);
    }
    tb.header(24.0, |mut h| {
        h.col(|ui| {
            header_cell(ui);
        });
        for (i, c) in t.columns.iter().enumerate() {
            let (_, resp) = h.col(|ui| header(ui, c, keys[i], &t.order));
            if resp.clicked() {
                ev.header_click = Some(i);
            }
        }
    })
    .body(|body| {
        let n_rows = t.rows.len();
        body.rows(22.0, n_rows, |mut row| grid_row(&mut row, t, editable, &mut ev));
    });
    apply(ui, t, ev, acts);
}

fn header(ui: &mut Ui, name: &str, key: bool, order: &Option<(String, bool)>) {
    let rect = header_cell(ui);
    ui.add_space(5.0);
    if key {
        let (r, _) = ui.allocate_exact_size(vec2(14.0, 14.0), Sense::hover());
        icons::paint(ui.painter(), r, Icon::Key);
        ui.add_space(2.0);
    }
    ui.add(egui::Label::new(RichText::new(name).strong()).truncate().selectable(false));
    if let Some((_, desc)) = order.as_ref().filter(|(oc, _)| oc == name) {
        arrow(ui.painter(), pos2(rect.right() - 10.0, rect.center().y), 4.0, if *desc { Arrow::Down } else { Arrow::Up }, pal().accent);
    }
}

fn grid_row(row: &mut TableRow, t: &mut DataTab, editable: bool, ev: &mut Events) {
    let r = row.index();
    row.col(|ui| row_marker(ui, t, r));
    for c in 0..t.columns.len() {
        let editing_here = t.editing.as_ref().is_some_and(|e| e.row == r && e.col == c);
        let (_, resp) = row.col(|ui| draw_cell(ui, t, r, c, editing_here));
        if editing_here {
            continue;
        }
        if resp.clicked() || resp.secondary_clicked() {
            t.commit_edit();
            t.sel = Some((r, c));
        }
        if resp.double_clicked() && editable {
            ev.start_edit = Some((r, c));
        }
        context_menu(&resp, |ui| {
            if let Some(cmd) = cell_menu::show(ui, editable) {
                ev.cmd = Some((r, c, cmd));
            }
        });
    }
}

/// Left gutter: deleted / new / changed / selected marker.
fn row_marker(ui: &mut Ui, t: &DataTab, r: usize) {
    let rect = header_cell(ui);
    let rs = &t.rows[r];
    let mr = Rect::from_center_size(pos2(rect.left() + 13.0, rect.center().y), vec2(12.0, 12.0));
    if rs.deleted {
        icons::paint(ui.painter(), mr, Icon::Delete);
    } else if rs.orig.is_none() {
        icons::paint(ui.painter(), mr, Icon::Add);
    } else if rs.changed() {
        ui.painter().circle_filled(mr.center(), 3.5, CHANGED_DOT);
    } else if matches!(t.sel, Some((sr, _)) if sr == r) {
        arrow(ui.painter(), mr.center(), 4.0, Arrow::Right, pal().accent);
    }
}

fn draw_cell(ui: &mut Ui, t: &mut DataTab, r: usize, c: usize, editing_here: bool) {
    let rect = ui.max_rect();
    let rs = &t.rows[r];
    let bg = if rs.deleted {
        Some(pal().deleted_bg)
    } else if rs.orig.is_none() {
        Some(pal().new_bg)
    } else if rs.orig.as_ref().is_some_and(|o| o[c] != rs.cur[c]) {
        Some(pal().edited_bg)
    } else {
        None
    };
    if let Some(bg) = bg {
        ui.painter().rect_filled(rect, 0.0, bg);
    }
    if t.sel == Some((r, c)) && !editing_here {
        ui.painter().rect_filled(rect, 0.0, pal().sel_bg);
    }
    cell_lines(ui, rect);
    if editing_here {
        cell_editor(ui, t, rect.width());
    } else {
        let color = if rs.deleted { pal().muted } else { pal().text };
        cell_text(ui, &rs.cur[c], color);
    }
}

/// In-place text editor; Escape cancels, leaving the cell saves.
fn cell_editor(ui: &mut Ui, t: &mut DataTab, width: f32) {
    let Some(edit) = &mut t.editing else { return };
    let te = ui.add(egui::TextEdit::singleline(&mut edit.text).desired_width(width - 2.0).margin(egui::Margin::symmetric(4, 1)));
    if edit.focus {
        te.request_focus();
        edit.focus = false;
    } else if te.lost_focus() {
        if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
            t.editing = None;
        } else {
            t.commit_edit();
        }
    }
}

/// Arrow keys / Tab move, Enter / F2 edit, Ctrl+C copies.
fn keyboard(ui: &Ui, t: &mut DataTab, editable: bool) {
    let no_focus = ui.ctx().memory(|m| m.focused().is_none());
    let Some((r, c)) = t.sel.filter(|_| t.editing.is_none() && no_focus) else { return };
    let last_row = t.rows.len().saturating_sub(1);
    let last_col = t.columns.len().saturating_sub(1);
    let (nr, nc) = ui.input(|i| {
        let down = i.key_pressed(egui::Key::ArrowDown);
        let up = i.key_pressed(egui::Key::ArrowUp);
        let right = i.key_pressed(egui::Key::ArrowRight) || i.key_pressed(egui::Key::Tab);
        let left = i.key_pressed(egui::Key::ArrowLeft);
        let nr = if down { (r + 1).min(last_row) } else if up { r.saturating_sub(1) } else { r };
        let nc = if right { (c + 1).min(last_col) } else if left { c.saturating_sub(1) } else { c };
        (nr, nc)
    });
    if (nr, nc) != (r, c) {
        t.sel = Some((nr, nc));
        t.scroll_to = Some(nr);
    }
    if editable && r < t.rows.len() && ui.input(|i| i.key_pressed(egui::Key::Enter) || i.key_pressed(egui::Key::F2)) {
        t.start_edit(r, c);
    }
    let copy = ui.input(|i| i.modifiers.command && i.key_pressed(egui::Key::C));
    if let (true, Some(v)) = (copy, t.rows.get(r).and_then(|row| row.cur.get(c))) {
        ui.ctx().copy_text(v.clone().unwrap_or_default());
    }
}

fn apply(ui: &Ui, t: &mut DataTab, ev: Events, acts: &mut Vec<Act>) {
    if let Some(i) = ev.header_click {
        t.toggle_order(i);
        reload(t, acts);
    }
    if let Some((r, c)) = ev.start_edit {
        t.sel = Some((r, c));
        t.start_edit(r, c);
    }
    if let Some((r, c, cmd)) = ev.cmd {
        cell_menu::apply(ui, t, (r, c), cmd, acts);
    }
}
