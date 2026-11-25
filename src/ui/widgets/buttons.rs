//! Toolbar and ribbon buttons, and their separators.

use eframe::egui::{self, pos2, vec2, FontId, Rect, Response, Sense, Stroke, Ui};

use crate::ui::icons::{self, Icon};
use crate::ui::theme::{self, pal};

pub fn toolbar_frame() -> egui::Frame {
    egui::Frame::new().fill(pal().toolbar_bg).inner_margin(egui::Margin::symmetric(6, 3))
}

/// Flat toolbar button with an optional icon; greyed out when disabled.
pub fn tool_btn(ui: &mut Ui, icon: Option<Icon>, text: &str, enabled: bool) -> Response {
    let color = if enabled { pal().text } else { pal().disabled };
    let galley = ui.painter().layout_no_wrap(text.to_owned(), FontId::proportional(12.5), color);
    let w = galley.size().x + if icon.is_some() { 41.0 } else { 20.0 };
    let (rect, resp) = ui.allocate_exact_size(vec2(w, 26.0), if enabled { Sense::click() } else { Sense::hover() });
    hover_frame(ui, rect, &resp, enabled);
    let p = ui.painter();
    let mut x = rect.left() + 10.0;
    if let Some(i) = icon {
        let ir = Rect::from_center_size(pos2(x + 8.0, rect.center().y), vec2(16.0, 16.0));
        icons::paint(p, ir, i);
        if !enabled {
            p.rect_filled(ir.expand(1.0), 0.0, theme::veil(pal().toolbar_bg));
        }
        x += 21.0;
    }
    p.galley(pos2(x, rect.center().y - galley.size().y / 2.0), galley, color);
    resp
}

/// Small square button with just an icon; `tip` names it on hover.
pub fn icon_btn(ui: &mut Ui, icon: Icon, tip: &str, enabled: bool) -> Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(24.0, 24.0), if enabled { Sense::click() } else { Sense::hover() });
    hover_frame(ui, rect, &resp, enabled);
    let p = ui.painter();
    let ir = Rect::from_center_size(rect.center(), vec2(14.0, 14.0));
    icons::paint(p, ir, icon);
    if !enabled {
        p.rect_filled(ir.expand(1.0), 0.0, theme::veil(pal().toolbar_bg));
    }
    resp.on_hover_text(tip)
}

fn hover_frame(ui: &Ui, rect: Rect, resp: &Response, enabled: bool) {
    if enabled && resp.hovered() {
        ui.painter().rect_filled(rect, 3.0, pal().hover_bg);
        ui.painter().rect_stroke(rect, 3.0, Stroke::new(1.0, pal().focus_border), egui::StrokeKind::Inside);
    }
}

/// Big icon-over-label button of the top ribbon.
pub fn ribbon_btn(ui: &mut Ui, icon: Icon, label: &str, selected: bool, enabled: bool) -> Response {
    let label_w = ui.painter().layout_no_wrap(label.to_owned(), FontId::proportional(12.0), pal().text).size().x;
    let (rect, resp) = ui.allocate_exact_size(vec2((label_w + 20.0).max(66.0), 60.0), if enabled { Sense::click() } else { Sense::hover() });
    let p = ui.painter();
    if selected {
        p.rect_filled(rect, 4.0, pal().sel_bg);
        p.rect_stroke(rect, 4.0, Stroke::new(1.0, pal().focus_border), egui::StrokeKind::Inside);
    } else if enabled && resp.hovered() {
        p.rect_filled(rect, 4.0, pal().hover_bg);
    }
    let ir = Rect::from_center_size(pos2(rect.center().x, rect.top() + 22.0), vec2(30.0, 30.0));
    icons::paint(p, ir, icon);
    if !enabled {
        p.rect_filled(ir.expand(2.0), 0.0, theme::veil(pal().ribbon_bg));
    }
    let color = if enabled { pal().text } else { pal().disabled };
    p.text(pos2(rect.center().x, rect.bottom() - 9.0), egui::Align2::CENTER_CENTER, label, FontId::proportional(12.0), color);
    resp
}

/// Vertical line between groups of buttons.
fn separator(ui: &mut Ui, width: f32, height: f32, inset: f32) {
    let (r, _) = ui.allocate_exact_size(vec2(width, height), Sense::hover());
    let x = r.center().x;
    ui.painter().line_segment([pos2(x, r.top() + inset), pos2(x, r.bottom() - inset)], Stroke::new(1.0, pal().border));
}

pub fn tool_sep(ui: &mut Ui) {
    separator(ui, 9.0, 22.0, 3.0);
}

pub fn ribbon_sep(ui: &mut Ui) {
    separator(ui, 12.0, 60.0, 8.0);
}
