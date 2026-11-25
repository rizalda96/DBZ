//! Data tab bottom bar: add/delete/apply/discard/refresh, last SQL and paging.

use eframe::egui::{self, RichText, Stroke, Ui};

use super::{reload, DataTab};
use crate::app::Act;
use crate::ui::icons::Icon;
use crate::ui::theme::pal;
use crate::ui::widgets::{tool_btn, toolbar_frame};
use crate::worker::{Purpose, Req};

pub(super) fn show(ui: &mut Ui, t: &mut DataTab, editable: bool, save_key: bool, acts: &mut Vec<Act>) {
    egui::Panel::bottom(("dbar", t.id)).exact_size(30.0).frame(toolbar_frame().stroke(Stroke::new(1.0, pal().border))).show(ui, |ui| {
        ui.horizontal_centered(|ui| {
            ui.spacing_mut().item_spacing.x = 1.0;
            record_buttons(ui, t, editable, save_key, acts);
            ui.add_space(10.0);
            ui.add(egui::Label::new(RichText::new(&t.last_sql).monospace().size(11.5).color(pal().muted)).truncate());
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| paging(ui, t, acts));
        });
    });
}

fn record_buttons(ui: &mut Ui, t: &mut DataTab, editable: bool, save_key: bool, acts: &mut Vec<Act>) {
    if tool_btn(ui, Some(Icon::Add), "", editable).on_hover_text("Add record").clicked() {
        t.add_row();
    }
    let delete = tool_btn(ui, Some(Icon::Minus), "", editable && t.sel.is_some()).on_hover_text("Delete record").clicked();
    if let (true, Some((r, _))) = (delete, t.sel) {
        t.delete_row(r, true);
    }
    let dirty = t.dirty();
    let apply = tool_btn(ui, Some(Icon::Check), "", dirty && !t.saving).on_hover_text("Apply changes (Ctrl+S)").clicked();
    if apply || (save_key && dirty) {
        save(t, acts);
    }
    if tool_btn(ui, Some(Icon::Discard), "", dirty).on_hover_text("Discard changes").clicked() {
        t.discard_changes();
    }
    if tool_btn(ui, Some(Icon::Refresh), "", !t.loading).on_hover_text("Refresh (F5)").clicked() {
        reload(t, acts);
    }
}

fn save(t: &mut DataTab, acts: &mut Vec<Act>) {
    t.commit_edit();
    let stmts = t.change_statements();
    if stmts.is_empty() {
        return;
    }
    t.saving = true;
    acts.push(Act::Send(t.cid, Req::Exec { tab: t.id, db: t.db.clone(), stmts, purpose: Purpose::SaveData }));
}

/// « ‹ page / pages › » and the record counter (laid out right to left).
fn paging(ui: &mut Ui, t: &mut DataTab, acts: &mut Vec<Act>) {
    let (page, pages) = t.pages();
    let (limit, offset) = (t.limit, t.offset);
    let mut go = |ui: &mut Ui, label: &str, enabled: bool, offset: usize, acts: &mut Vec<Act>| {
        if ui.add_enabled(enabled, egui::Button::new(label).small()).clicked() {
            t.offset = offset;
            reload(t, acts);
        }
    };
    go(ui, "»", page < pages, (pages - 1) * limit, acts);
    go(ui, "›", page < pages, offset + limit, acts);
    ui.label(format!(" {} / {} ", page, pages));
    go(ui, "‹", page > 1, offset.saturating_sub(limit), acts);
    go(ui, "«", page > 1, 0, acts);
    ui.add_space(10.0);
    let rec = match t.sel {
        Some((r, _)) => format!("Record {} of {}", t.offset + r + 1, t.total_rows()),
        None => format!("{} record(s)", t.total_rows()),
    };
    ui.label(RichText::new(rec).color(pal().muted));
}
