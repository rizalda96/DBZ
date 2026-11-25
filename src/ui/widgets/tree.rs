//! One row of the connection tree.

use eframe::egui::{self, pos2, vec2, FontId, Rect, Response, Sense, Stroke, Ui};

use crate::ui::icons::{self, Icon};
use crate::ui::theme::pal;

const INDENT: f32 = 16.0;

/// What one tree row shows.
pub struct TreeItem<'a> {
    pub depth: usize,
    /// Some(open) for rows that can expand.
    pub expander: Option<bool>,
    pub icon: Icon,
    pub label: &'a str,
    pub selected: bool,
}

/// Draws a tree row. Returns the row response and whether the expander was toggled
/// (click on the triangle or a double click).
pub fn tree_row(ui: &mut Ui, item: TreeItem) -> (Response, bool) {
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 22.0), Sense::click());
    let p = ui.painter();
    if item.selected {
        p.rect_filled(rect, 0.0, pal().sel_bg);
    } else if resp.hovered() {
        p.rect_filled(rect, 0.0, pal().tree_hover);
    }
    let x0 = rect.left() + 4.0 + item.depth as f32 * INDENT;
    let toggled = item.expander.is_some_and(|open| draw_expander(p, &resp, pos2(x0 + 6.0, rect.center().y), open));
    icons::paint(p, Rect::from_center_size(pos2(x0 + 22.0, rect.center().y), vec2(16.0, 16.0)), item.icon);
    p.with_clip_rect(rect).text(pos2(x0 + 34.0, rect.center().y), egui::Align2::LEFT_CENTER, item.label, FontId::proportional(13.0), pal().text);
    (resp, toggled)
}

/// Draws the triangle at `c`; true when it was clicked or the row double-clicked.
fn draw_expander(p: &egui::Painter, resp: &Response, c: egui::Pos2, open: bool) -> bool {
    let pts = if open {
        vec![pos2(c.x - 4.0, c.y - 2.0), pos2(c.x + 4.0, c.y - 2.0), pos2(c.x, c.y + 3.0)]
    } else {
        vec![pos2(c.x - 2.0, c.y - 4.0), pos2(c.x + 3.0, c.y), pos2(c.x - 2.0, c.y + 4.0)]
    };
    p.add(egui::Shape::convex_polygon(pts, pal().expander, Stroke::NONE));
    let on_triangle = resp.clicked() && resp.interact_pointer_pos().is_some_and(|pos| pos.x < c.x + 8.0);
    on_triangle || resp.double_clicked()
}
