//! The index fields editor: a panel that drops down under an index's Fields cell and
//! lists its columns, each picked from the table's fields, with prefix length,
//! collation, operator class and sort order where the server has them.

use eframe::egui::{vec2, Ui};

use super::cell_window::{cell_window, WindowKey};
use super::index_cols_state::{ColsTarget, IndexColsEditor};
use super::parts_grid::{grid, Cell, GridSpec};
use super::DesignTab;
use crate::domain::{IndexChoices, IndexColumn};
use crate::sql::{DesignerCaps, NULLS_ORDERS, SORT_ORDERS};
use crate::ui::icons::Icon;
use crate::ui::widgets::{buttons, icon_btn, Choice};

const SIZE: [f32; 2] = [830.0, 300.0];

/// The window's key for [`cell_window`] and [`super::cell_window::anchor_at`].
pub(super) fn window_key(tab: u64) -> WindowKey {
    ("ixcols", tab)
}

/// Draws the editor while it is open; OK writes the columns back to their row.
pub(super) fn show(ui: &Ui, t: &mut DesignTab) {
    let d = t.dialect();
    let caps = d.designer();
    let fields = t.field_refs();
    let id = t.id;
    let Some(ed) = t.parts.cols.as_mut() else { return };
    let lists = Lists { fields: &fields, choices: &t.index_choices, method: ed.method.clone(), operators: matches!(ed.target, ColsTarget::Exclude(_)) };
    let title = if lists.operators { "Exclude Elements" } else { "Index Fields" };
    let choice = cell_window(ui, window_key(id), title, SIZE, |ui| body(ui, ed, (id, caps), &lists));
    if choice.keep() {
        return;
    }
    t.parts.close_cols(choice == Choice::Ok, d);
}

/// What the drop-downs offer.
struct Lists<'a> {
    /// The table's fields, as written in the column list.
    fields: &'a [String],
    choices: &'a IndexChoices,
    /// The index method, which decides the operator classes.
    method: String,
    /// Exclusion constraint elements also take an operator.
    operators: bool,
}

fn body(ui: &mut Ui, ed: &mut IndexColsEditor, (id, caps): (u64, &'static DesignerCaps), lists: &Lists) -> Choice {
    let grid_size = vec2(ui.available_width(), ui.available_height() - 40.0);
    ui.allocate_ui(grid_size, |ui| columns_grid(ui, ed, (id, caps), lists));
    let mut choice = Choice::None;
    ui.horizontal(|ui| {
        row_buttons(ui, ed);
        choice = buttons(ui, "OK", true);
    });
    choice
}

fn columns_grid(ui: &mut Ui, ed: &mut IndexColsEditor, (id, caps): (u64, &'static DesignerCaps), lists: &Lists) {
    let mut columns = vec![("Name", 180.0)];
    if caps.index_sub_part {
        columns.push(("Sub Part", 80.0));
    }
    if caps.index_collation_opclass {
        columns.extend([("Collation Schema", 120.0), ("Collation", 100.0), ("Operator Class Schema", 150.0), ("Operator Class", 120.0)]);
    }
    columns.push(("Sort Order", 90.0));
    if caps.index_collation_opclass {
        columns.push(("Nulls Order", 110.0));
    }
    if lists.operators {
        columns.push(("Operator", 90.0));
    }
    let spec = GridSpec { id: ("ixcols", id), columns: &columns };
    grid(ui, &spec, &mut ed.rows, &mut ed.sel, |c: &mut IndexColumn| column_cells(c, caps, lists));
}

/// The same columns, in the same order, as `columns_grid` lists. Collations and
/// operator classes follow the schema picked in the row.
fn column_cells<'b>(c: &'b mut IndexColumn, caps: &DesignerCaps, lists: &Lists) -> Vec<Cell<'b>> {
    let mut out = vec![Cell::Pick(&mut c.name, lists.fields.to_vec())];
    if caps.index_sub_part {
        out.push(Cell::Text(&mut c.sub_part));
    }
    if caps.index_collation_opclass {
        let ch = lists.choices;
        let collations = ch.collations_in(&c.collation_schema);
        let opclasses = ch.opclasses_for(&c.opclass_schema, &lists.method);
        out.extend([
            Cell::Pick(&mut c.collation_schema, optional(&ch.schemas)),
            Cell::Pick(&mut c.collation, optional(&collations)),
            Cell::Pick(&mut c.opclass_schema, optional(&ch.schemas)),
            Cell::Pick(&mut c.opclass, optional(&opclasses)),
        ]);
    }
    out.push(Cell::Choice(&mut c.order, &SORT_ORDERS));
    if caps.index_collation_opclass {
        out.push(Cell::Choice(&mut c.nulls, &NULLS_ORDERS));
    }
    if lists.operators {
        out.push(Cell::Pick(&mut c.operator, lists.choices.operators.clone()));
    }
    out
}

/// A list led by an empty entry, so an optional value can be cleared again.
fn optional(list: &[String]) -> Vec<String> {
    std::iter::once(String::new()).chain(list.iter().cloned()).collect()
}

/// Move up / down, add and remove, bottom left.
fn row_buttons(ui: &mut Ui, ed: &mut IndexColsEditor) {
    if icon_btn(ui, Icon::MoveUp, "Move Up", ed.can_move(-1)).clicked() {
        ed.move_sel(-1);
    }
    if icon_btn(ui, Icon::MoveDown, "Move Down", ed.can_move(1)).clicked() {
        ed.move_sel(1);
    }
    if icon_btn(ui, Icon::Add, "Add", true).clicked() {
        ed.add();
    }
    if icon_btn(ui, Icon::Minus, "Remove", ed.sel.is_some()).clicked() {
        ed.remove();
    }
}
