//! The designer's Foreign Keys tab: the grid, where fields and referenced fields are
//! picked with tick boxes and the referenced schema and table from drop-downs, and
//! the MATCH / DEFERRABLE panel under it on servers that have those.

use eframe::egui::{self, RichText, Stroke, Ui};

use super::form_rows::deferrable_rows;
use super::parts_grid::{grid, Cell, GridSpec};
use super::parts_state::PartKind;
use super::ref_state::RefColumns;
use super::DesignTab;
use crate::domain::{ForeignKeyDef, IndexChoices};
use crate::sql::FK_ACTIONS;
use crate::ui::theme::pal;

const COLUMNS: [(&str, f32); 8] =
    [("Name", 170.0), ("Fields", 150.0), ("Referenced Schema", 140.0), ("Referenced Table", 150.0), ("Referenced Fields", 150.0), ("On Delete", 110.0), ("On Update", 110.0), ("Comment", 140.0)];

/// What the drop-downs offer.
pub(super) struct FkLists<'a> {
    /// This table's fields.
    pub fields: &'a [String],
    pub choices: &'a IndexChoices,
    pub refs: &'a RefColumns,
    pub comments: bool,
}

/// Before drawing: asks for the columns of the tables the keys reference.
pub(super) fn want_ref_columns(t: &mut DesignTab) {
    for f in &t.parts.parts.foreign_keys {
        t.ref_columns.want(&f.ref_schema, &f.ref_table);
    }
}

pub(super) fn fk_grid(ui: &mut Ui, (id, sel): ((&'static str, u64), &mut Option<usize>), fks: &mut [ForeignKeyDef], lists: &FkLists) {
    grid(ui, &GridSpec { id, columns: &COLUMNS }, fks, sel, |f: &mut ForeignKeyDef| fk_cells(f, lists));
}

/// Referenced tables follow the schema, referenced fields follow the table.
fn fk_cells<'b>(f: &'b mut ForeignKeyDef, lists: &FkLists) -> Vec<Cell<'b>> {
    let tables = lists.choices.tables_in(&f.ref_schema);
    let ref_fields = lists.refs.get(&f.ref_schema, &f.ref_table);
    vec![
        Cell::Text(&mut f.name),
        Cell::Multi(&mut f.columns, lists.fields.to_vec()),
        Cell::Pick(&mut f.ref_schema, lists.choices.table_schemas()),
        Cell::Pick(&mut f.ref_table, tables),
        Cell::Multi(&mut f.ref_columns, ref_fields),
        Cell::Choice(&mut f.on_delete, &FK_ACTIONS),
        Cell::Choice(&mut f.on_update, &FK_ACTIONS),
        if lists.comments { Cell::Text(&mut f.comment) } else { Cell::Off },
    ]
}

/// Match Full, Deferrable and Deferred of the selected foreign key.
pub(super) fn options_panel(ui: &mut Ui, t: &mut DesignTab) {
    let id = t.id;
    let sel = t.parts.sel(PartKind::ForeignKey);
    let fk = sel.and_then(|i| t.parts.parts.foreign_keys.get_mut(i));
    let frame = egui::Frame::new().fill(pal().toolbar_bg).inner_margin(10).stroke(Stroke::new(1.0, pal().border));
    egui::Panel::bottom(("dfk", id)).resizable(true).default_size(120.0).frame(frame).show(ui, |ui| {
        let Some(fk) = fk else {
            ui.label(RichText::new("Select a foreign key to see its options.").color(pal().muted));
            return;
        };
        fk_options(ui, id, fk);
    });
}

fn fk_options(ui: &mut Ui, id: u64, fk: &mut ForeignKeyDef) {
    ui.checkbox(&mut fk.match_full, "Match Full");
    ui.add_space(6.0);
    egui::Grid::new(("dfk_opts", id)).num_columns(2).spacing([24.0, 8.0]).min_col_width(140.0).show(ui, |ui| {
        deferrable_rows(ui, ("dfk", id), &mut fk.deferrable, &mut fk.deferred);
    });
}
