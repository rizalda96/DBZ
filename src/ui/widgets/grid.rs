//! Pieces shared by every grid: cell borders, cell text, header cells, column widths
//! and the little arrows.

use eframe::egui::{self, pos2, Color32, Pos2, Rect, RichText, Stroke, Ui};

use crate::domain::{Cell, Row};
use crate::ui::theme::pal;

const NULL_TEXT: &str = "(Null)";
const MAX_CELL_CHARS: usize = 300;

/// Direction of a small filled triangle.
#[derive(Clone, Copy)]
pub enum Arrow {
    Right,
    Down,
    Up,
}

pub fn arrow(p: &egui::Painter, c: Pos2, r: f32, dir: Arrow, color: Color32) {
    let pts = match dir {
        Arrow::Right => vec![pos2(c.x - r * 0.6, c.y - r), pos2(c.x + r * 0.8, c.y), pos2(c.x - r * 0.6, c.y + r)],
        Arrow::Down => vec![pos2(c.x - r, c.y - r * 0.6), pos2(c.x + r, c.y - r * 0.6), pos2(c.x, c.y + r * 0.8)],
        Arrow::Up => vec![pos2(c.x - r, c.y + r * 0.6), pos2(c.x + r, c.y + r * 0.6), pos2(c.x, c.y - r * 0.8)],
    };
    p.add(egui::Shape::convex_polygon(pts, color, Stroke::NONE));
}

/// Right and bottom border of a cell.
pub fn cell_lines(ui: &Ui, rect: Rect) {
    let p = ui.painter();
    let stroke = Stroke::new(1.0, pal().grid_line);
    p.line_segment([rect.right_top(), rect.right_bottom()], stroke);
    p.line_segment([rect.left_bottom(), rect.right_bottom()], stroke);
}

/// Header-coloured background with borders; returns the cell rect.
pub fn header_cell(ui: &mut Ui) -> Rect {
    let rect = ui.max_rect();
    ui.painter().rect_filled(rect, 0.0, pal().header_bg);
    cell_lines(ui, rect);
    rect
}

/// One-line cell value; NULL shows as a muted "(Null)".
pub fn cell_text(ui: &mut Ui, v: &Cell, color: Color32) {
    ui.add_space(5.0);
    let text = match v {
        None => RichText::new(NULL_TEXT).italics().color(pal().muted),
        Some(s) => RichText::new(one_line(s)).color(color),
    };
    ui.add(egui::Label::new(text).truncate().selectable(false));
}

fn one_line(s: &str) -> String {
    if !s.contains('\n') && s.len() <= MAX_CELL_CHARS {
        return s.to_string();
    }
    s.chars().take(MAX_CELL_CHARS).map(|c| if c == '\n' || c == '\r' { ' ' } else { c }).collect()
}

/// A starting column width that fits the header and the first rows.
pub fn col_width(name: &str, rows: &[Row], idx: usize) -> f32 {
    let header = name.chars().count() as f32 * 7.5 + 30.0;
    let widest = rows.iter().take(50).filter_map(|r| r.get(idx).cloned().flatten()).map(|v| v.chars().count().min(60) as f32 * 7.0 + 16.0).fold(header, f32::max);
    widest.clamp(60.0, 360.0)
}
