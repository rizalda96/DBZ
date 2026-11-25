//! An editable grid for the designer's Indexes, Foreign Keys, Uniques, Checks and
//! Triggers tabs (and the index fields editor): one row per item, one cell per
//! property. Cells show plain text; the clicked one turns into its editor.

use eframe::egui::{self, pos2, vec2, Rect, RichText, Sense, Ui};
use egui_extras::{Column, TableBuilder, TableRow};

use super::parts_cells::{draw_cell, CellState, Hit};
use crate::ui::theme::pal;
use crate::ui::widgets::{arrow, cell_lines, header_cell, Arrow};

/// One editable property of a row.
pub(super) enum Cell<'a> {
    Text(&'a mut String),
    /// A drop-down of these values, worked out per row (e.g. the table's fields).
    Pick(&'a mut String, Vec<String>),
    /// A drop-down with tick boxes; the value lists the ticked ones, "a, b".
    Multi(&'a mut String, Vec<String>),
    /// A value from a fixed list.
    Choice(&'a mut String, &'static [&'static str]),
    Check(&'a mut bool),
    /// Read-only text with a "…" button that opens an editor; the grid reports the click.
    Editor(&'a str),
    /// Read-only text.
    Label(&'a str),
    /// A property this server doesn't have.
    Off,
}

impl Cell<'_> {
    /// Cells that become a text box when clicked.
    fn typed(&self) -> bool {
        matches!(self, Cell::Text(_))
    }
}

/// Grid id, column headers with their starting widths.
pub(super) struct GridSpec<'a> {
    pub id: (&'static str, u64),
    pub columns: &'a [(&'a str, f32)],
}

/// The cell being edited, kept in egui memory per grid.
#[derive(Clone, Copy)]
struct Active {
    row: usize,
    col: usize,
    /// Focus its text box this frame (the one after the click).
    focus: bool,
}

/// Draws `items`; clicking or typing in a row selects it. Returns the row whose
/// `Cell::Editor` was opened, with that cell's rect.
pub(super) fn grid<T>(ui: &mut Ui, spec: &GridSpec, items: &mut [T], sel: &mut Option<usize>, cells: impl for<'b> Fn(&'b mut T) -> Vec<Cell<'b>>) -> Option<(usize, Rect)> {
    // TableBuilder only scrolls vertically; wide grids need an outer horizontal scroll.
    egui::ScrollArea::horizontal().id_salt(("parts_h", spec.id)).auto_shrink(false).show(ui, |ui| table(ui, spec, items, sel, &cells)).inner
}

/// What all rows reported this frame.
#[derive(Default)]
struct Hits {
    touched: Option<usize>,
    opened: Option<(usize, Rect)>,
    activated: Option<(usize, usize)>,
    done: bool,
}

impl Hits {
    fn add(&mut self, i: usize, h: Hit) {
        self.touched = self.touched.or(h.touched.then_some(i));
        self.opened = self.opened.or(h.opened.map(|r| (i, r)));
        self.activated = self.activated.or(h.activate.map(|c| (i, c)));
        self.done |= h.done;
    }
}

fn table<T>(ui: &mut Ui, spec: &GridSpec, items: &mut [T], sel: &mut Option<usize>, cells: &impl for<'b> Fn(&'b mut T) -> Vec<Cell<'b>>) -> Option<(usize, Rect)> {
    let memory = egui::Id::unique(("parts_active", spec.id));
    let active: Option<Active> = ui.data(|d| d.get_temp(memory));
    ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
    let last = spec.columns.len().saturating_sub(1);
    let mut tb = TableBuilder::new(ui)
        .id_salt(spec.id)
        .striped(true)
        .resizable(true)
        .auto_shrink(false)
        .sense(Sense::click())
        .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
        .column(Column::exact(28.0));
    for (i, (_, width)) in spec.columns.iter().enumerate() {
        tb = tb.column(if i == last { Column::remainder().at_least(*width) } else { Column::initial(*width).at_least(40.0).clip(true) });
    }
    let selected = *sel;
    let mut hits = Hits::default();
    tb.header(24.0, |mut h| {
        h.col(|ui| {
            header_cell(ui);
        });
        for (title, _) in spec.columns {
            h.col(|ui| header(ui, title));
        }
    })
    .body(|body| {
        body.rows(26.0, items.len(), |mut row| {
            let i = row.index();
            let mine = active.filter(|a| a.row == i);
            hits.add(i, part_row(&mut row, (spec.id, i), (selected == Some(i), mine), cells(&mut items[i])));
        });
    });
    if hits.touched.is_some() {
        *sel = hits.touched;
    }
    remember(ui, memory, next_active(active, &hits));
    hits.opened
}

/// A new click wins; Enter / Escape / picking a value ends the edit.
fn next_active(active: Option<Active>, hits: &Hits) -> Option<Active> {
    match (hits.activated, hits.done) {
        (Some((row, col)), _) => Some(Active { row, col, focus: true }),
        (None, true) => None,
        (None, false) => active.map(|a| Active { focus: false, ..a }),
    }
}

fn remember(ui: &Ui, memory: egui::Id, active: Option<Active>) {
    ui.data_mut(|d| match active {
        Some(a) => {
            d.insert_temp(memory, a);
        }
        None => {
            d.remove::<Active>(memory);
        }
    });
}

fn header(ui: &mut Ui, title: &str) {
    header_cell(ui);
    ui.add_space(5.0);
    ui.label(RichText::new(title).strong());
}

fn part_row(row: &mut TableRow, key: ((&'static str, u64), usize), (selected, active): (bool, Option<Active>), cells: Vec<Cell>) -> Hit {
    row.set_selected(selected);
    let (_, marker) = row.col(|ui| {
        let rect = header_cell(ui);
        if selected {
            arrow(ui.painter(), pos2(rect.left() + 13.0, rect.center().y), 4.0, Arrow::Right, pal().accent);
        }
    });
    let mut hit = Hit { touched: marker.clicked(), ..Default::default() };
    for (c, cell) in cells.into_iter().enumerate() {
        let (typed, editor) = (cell.typed(), matches!(cell, Cell::Editor(_)));
        let state = CellState { active: active.is_some_and(|a| a.col == c), focus: active.is_some_and(|a| a.col == c && a.focus), selected };
        let (_, resp) = row.col(|ui| {
            cell_lines(ui, ui.max_rect());
            hit.add(draw_cell(ui, (key, c), cell, state));
        });
        // A click on the cell's plain text lands on the cell itself.
        let clicked = resp.clicked();
        hit.add(Hit { touched: clicked, activate: (clicked && typed).then_some(c), opened: (editor && resp.double_clicked()).then_some(resp.rect), done: false });
    }
    hit
}
