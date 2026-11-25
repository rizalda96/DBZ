//! The tab strip above the work area.

use eframe::egui::{self, pos2, vec2, FontId, Rect, Response, Sense, Stroke, Ui};

use crate::app::{Act, Workspace};
use crate::ui::icons::{self, Icon};
use crate::ui::theme::pal;
use crate::ui::widgets::{context_menu, menu_item};

const FONT_SIZE: f32 = 12.5;

struct TabInfo {
    index: usize,
    title: String,
    icon: Icon,
    id: u64,
}

pub fn tab_bar(ui: &mut Ui, ws: &Workspace, acts: &mut Vec<Act>) {
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 31.0), Sense::hover());
    ui.painter().rect_filled(rect, 0.0, pal().tab_strip);
    ui.painter().line_segment([rect.left_bottom(), rect.right_bottom()], Stroke::new(1.0, pal().border));
    let objects = TabInfo { index: 0, title: "Objects".into(), icon: Icon::Folder, id: 0 };
    let tabs = ws.tabs.iter().enumerate().map(|(i, t)| TabInfo { index: i + 1, title: t.title(&ws.conns), icon: t.icon(), id: t.id() });
    let max_w = ((rect.width() - 8.0) / (ws.tabs.len() + 1) as f32).clamp(90.0, 280.0);
    let mut x = rect.left() + 4.0;
    for tab in std::iter::once(objects).chain(tabs) {
        let galley = ui.painter().layout_no_wrap(tab.title.clone(), FontId::proportional(FONT_SIZE), pal().text);
        let w = (galley.size().x + if tab.index == 0 { 40.0 } else { 58.0 }).min(max_w);
        if x + w > rect.right() {
            break;
        }
        let tr = Rect::from_min_size(pos2(x, rect.top() + 4.0), vec2(w, rect.height() - 4.0));
        draw_tab(ui, tr, &tab, ws.active == tab.index, acts);
        x += w + 2.0;
    }
}

fn draw_tab(ui: &mut Ui, tr: Rect, tab: &TabInfo, active: bool, acts: &mut Vec<Act>) {
    let resp = ui.interact(tr, ui.scope_id().with(("tab", tab.id)), Sense::click());
    let p = ui.painter().with_clip_rect(tr);
    let fill = if active { pal().surface } else if resp.hovered() { pal().tab_hover } else { pal().tab_inactive };
    p.rect_filled(tr, egui::CornerRadius { nw: 4, ne: 4, sw: 0, se: 0 }, fill);
    if active {
        p.rect_filled(Rect::from_min_size(tr.min, vec2(tr.width(), 2.5)), 0.0, pal().accent);
    }
    icons::paint(&p, Rect::from_center_size(pos2(tr.left() + 15.0, tr.center().y), vec2(15.0, 15.0)), tab.icon);
    let text_clip = Rect::from_min_max(tr.min, pos2(tr.right() - if tab.index == 0 { 4.0 } else { 26.0 }, tr.bottom()));
    let color = if active { pal().text } else { pal().text_dim };
    ui.painter().with_clip_rect(text_clip).text(pos2(tr.left() + 28.0, tr.center().y), egui::Align2::LEFT_CENTER, &tab.title, FontId::proportional(FONT_SIZE), color);
    let closed = tab.index > 0 && close_button(&p, tr, &resp, active);
    if closed {
        acts.push(Act::CloseTab(tab.index));
    } else if resp.clicked() {
        acts.push(Act::Activate(tab.index));
    }
    if tab.index > 0 {
        context_menu(&resp.on_hover_text(&tab.title), |ui| menu_item(ui, acts, "Close", Act::CloseTab(tab.index)));
    }
}

/// The "×" of a work tab; true when the tab should close (× click or middle click).
fn close_button(p: &egui::Painter, tr: Rect, resp: &Response, active: bool) -> bool {
    let cr = Rect::from_center_size(pos2(tr.right() - 13.0, tr.center().y), vec2(16.0, 16.0));
    let hover_close = resp.hover_pos().is_some_and(|pos| cr.contains(pos));
    if hover_close {
        p.rect_filled(cr, 3.0, pal().tab_close_hover);
    }
    if active || resp.hovered() {
        p.text(cr.center(), egui::Align2::CENTER_CENTER, "×", FontId::proportional(15.0), pal().muted);
    }
    (resp.clicked() && hover_close) || resp.middle_clicked()
}
