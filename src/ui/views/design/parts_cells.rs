//! How one cell of the designer's parts grid draws: plain text until clicked, then a
//! text box (with a drop-down arrow for picked values); fixed lists drop down at once.

use eframe::egui::{self, pos2, vec2, Rect, RichText, Sense, Stroke, Ui, UiBuilder};

use super::parts_grid::Cell;
use crate::ui::theme::pal;
use crate::ui::widgets::{arrow, cell_text, Arrow};

pub(super) type CellKey = (((&'static str, u64), usize), usize);

/// What the user did in a cell (or a row) this frame.
#[derive(Default, Clone, Copy)]
pub(super) struct Hit {
    /// Clicked or focused something, so the row becomes selected.
    pub touched: bool,
    /// Column to edit from now on.
    pub activate: Option<usize>,
    /// The edit ended (Enter, Escape, a value picked).
    pub done: bool,
    /// A `Cell::Editor` asked for its editor; the cell's rect.
    pub opened: Option<Rect>,
}

impl Hit {
    pub fn add(&mut self, other: Hit) {
        self.touched |= other.touched;
        self.activate = self.activate.or(other.activate);
        self.done |= other.done;
        self.opened = self.opened.or(other.opened);
    }
}

#[derive(Clone, Copy)]
pub(super) struct CellState {
    /// This cell is being edited.
    pub active: bool,
    /// Give its text box focus now.
    pub focus: bool,
    /// Its row is selected.
    pub selected: bool,
}

const ARROW_W: f32 = 20.0;
/// How a drop-down lists its empty choice (clears the value).
const NONE_LABEL: &str = "(none)";

pub(super) fn draw_cell(ui: &mut Ui, key: CellKey, cell: Cell, st: CellState) -> Hit {
    match cell {
        Cell::Text(s) if st.active => text_box(ui, key, s, st.focus),
        Cell::Text(s) => {
            cell_text(ui, &Some(s.clone()), pal().text);
            Hit::default()
        }
        Cell::Pick(s, options) => dropdown(ui, key, s, options.iter().map(String::as_str), st.active),
        Cell::Choice(s, options) => dropdown(ui, key, s, options.iter().copied(), st.active),
        Cell::Check(b) => {
            ui.add_space(8.0);
            Hit { touched: ui.checkbox(b, "").clicked(), ..Default::default() }
        }
        Cell::Editor(text) => editor(ui, text, st.selected),
        Cell::Off => Hit::default(),
    }
}

/// The text box of the cell being edited.
fn text_box(ui: &mut Ui, key: CellKey, s: &mut String, focus: bool) -> Hit {
    ui.add_space(2.0);
    let te = ui.add(egui::TextEdit::singleline(s).desired_width(ui.available_width() - 6.0).id_salt(key));
    if focus {
        te.request_focus();
    }
    let ended = te.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter) || i.key_pressed(egui::Key::Escape));
    Hit { touched: te.gained_focus(), done: ended, ..Default::default() }
}

/// A drop-down: the whole cell is its button, so one click opens the list right under
/// the cell. The value can only be picked, not typed. A current value that isn't in
/// the list (e.g. an expression loaded from the server) still shows and stays.
fn dropdown<'o>(ui: &mut Ui, key: CellKey, value: &mut String, options: impl Iterator<Item = &'o str>, active: bool) -> Hit {
    let cell = ui.max_rect();
    let resp = ui.interact(cell, egui::Id::unique(("dropdown", key)), Sense::click());
    if active {
        ui.painter().rect_stroke(cell.shrink(1.0), 0.0, Stroke::new(1.0, pal().focus_border), egui::StrokeKind::Inside);
    }
    cell_text(ui, &Some(value.clone()), pal().text);
    if active || resp.hovered() {
        arrow_mark(ui, arrow_rect(cell), resp.hovered());
    }
    let picked = egui::Popup::menu(&resp).anchor(cell).width(cell.width()).show(|ui| option_list(ui, value, options)).is_some_and(|r| r.inner);
    Hit { touched: resp.clicked() || picked, activate: resp.clicked().then_some(key.1), done: picked, opened: None }
}

fn arrow_rect(cell: Rect) -> Rect {
    Rect::from_min_max(pos2(cell.right() - ARROW_W - 3.0, cell.top() + 3.0), pos2(cell.right() - 3.0, cell.bottom() - 3.0))
}

/// The small boxed ▾ at a drop-down cell's right edge.
fn arrow_mark(ui: &Ui, rect: Rect, hovered: bool) {
    let p = ui.painter();
    p.rect_filled(rect, 2.0, if hovered { pal().hover_bg } else { pal().header_bg });
    p.rect_stroke(rect, 2.0, Stroke::new(1.0, pal().border), egui::StrokeKind::Inside);
    arrow(p, rect.center(), 4.0, Arrow::Down, pal().text);
}

/// The drop-down's entries; true when one was picked.
fn option_list<'o>(ui: &mut Ui, value: &mut String, options: impl Iterator<Item = &'o str>) -> bool {
    let mut picked = false;
    let mut any = false;
    egui::ScrollArea::vertical().max_height(240.0).show(ui, |ui| {
        for o in options {
            any = true;
            let label = if o.is_empty() { RichText::new(NONE_LABEL).color(pal().muted) } else { RichText::new(o) };
            if ui.selectable_label(value.as_str() == o, label).clicked() {
                *value = o.to_string();
                picked = true;
            }
        }
    });
    if !any {
        ui.label(RichText::new("Nothing to pick").color(pal().muted));
    }
    picked
}

/// Read-only text; the selected row also shows a "…" button that opens the editor.
fn editor(ui: &mut Ui, text: &str, selected: bool) -> Hit {
    let cell = ui.max_rect();
    let button = Rect::from_min_max(pos2(cell.right() - 26.0, cell.top() + 3.0), pos2(cell.right() - 4.0, cell.bottom() - 3.0));
    let right = if selected { button.left() - 4.0 } else { cell.right() };
    let text_rect = Rect::from_min_max(cell.min, pos2(right, cell.bottom()));
    let layout = egui::Layout::left_to_right(egui::Align::Center);
    ui.scope_builder(UiBuilder::new().max_rect(text_rect).layout(layout), |ui| cell_text(ui, &Some(text.to_string()), pal().text));
    if !selected {
        return Hit::default();
    }
    let opened = ui.put(button, egui::Button::new("…").min_size(vec2(22.0, 18.0))).on_hover_text("Edit").clicked();
    Hit { touched: opened, opened: opened.then_some(cell), ..Default::default() }
}
