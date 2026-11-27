//! Right-click menu of a data grid cell.

use eframe::egui::{self, Ui};

use super::{reload, DataTab};
use crate::app::Act;

#[derive(Clone, Copy)]
pub(super) enum CellCmd {
    Edit,
    SetNull,
    SetEmpty,
    Copy,
    CopyInsert,
    Filter,
    AddRow,
    DeleteRow,
    Refresh,
}

const EDIT_ITEMS: [(&str, CellCmd); 3] = [("Edit Cell", CellCmd::Edit), ("Set to NULL", CellCmd::SetNull), ("Set to Empty String", CellCmd::SetEmpty)];
const COPY_ITEMS: [(&str, CellCmd); 3] = [("Copy", CellCmd::Copy), ("Copy as INSERT Statement", CellCmd::CopyInsert), ("Filter by This Value", CellCmd::Filter)];
const ROW_ITEMS: [(&str, CellCmd); 2] = [("Add Record", CellCmd::AddRow), ("Delete Record", CellCmd::DeleteRow)];

/// Draws the menu; returns the chosen command.
pub(super) fn show(ui: &mut Ui, editable: bool) -> Option<CellCmd> {
    let mut chosen = None;
    if editable {
        chosen = chosen.or(items(ui, &EDIT_ITEMS));
        ui.separator();
    }
    chosen = chosen.or(items(ui, &COPY_ITEMS));
    if editable {
        ui.separator();
        chosen = chosen.or(items(ui, &ROW_ITEMS));
    }
    ui.separator();
    if ui.add(egui::Button::new("Refresh").shortcut_text("F5")).clicked() {
        chosen = Some(CellCmd::Refresh);
        ui.close();
    }
    chosen
}

fn items(ui: &mut Ui, list: &[(&str, CellCmd)]) -> Option<CellCmd> {
    let mut chosen = None;
    for (label, cmd) in list {
        if ui.button(*label).clicked() {
            chosen = Some(*cmd);
            ui.close();
        }
    }
    chosen
}

pub(super) fn apply(ui: &Ui, t: &mut DataTab, (r, c): (usize, usize), cmd: CellCmd, acts: &mut Vec<Act>) {
    match cmd {
        CellCmd::Edit => t.start_edit(r, c),
        CellCmd::SetNull => t.rows[r].cur[c] = None,
        CellCmd::SetEmpty => t.rows[r].cur[c] = Some(String::new()),
        CellCmd::Copy => ui.ctx().copy_text(t.rows[r].cur[c].clone().unwrap_or_default()),
        CellCmd::CopyInsert => ui.ctx().copy_text(t.insert_sql(r)),
        CellCmd::Filter => {
            t.filter_by(r, c);
            reload(t, acts);
        }
        CellCmd::AddRow => t.add_row(),
        CellCmd::DeleteRow => t.delete_row(r, false),
        CellCmd::Refresh => reload(t, acts),
    }
}
