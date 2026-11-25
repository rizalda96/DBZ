//! Designer toolbar: save, field editing and the table name.

use eframe::egui::{self, Ui};

use super::parts_state::PartKind;
use super::state::DesignSub;
use super::DesignTab;
use crate::app::Act;
use crate::ui::icons::Icon;
use crate::ui::widgets::{tool_btn, tool_sep, toolbar_frame};
use crate::worker::{Purpose, Req};

pub(super) fn show(ui: &mut Ui, t: &mut DesignTab, save_key: bool, acts: &mut Vec<Act>) {
    toolbar_frame().show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 2.0;
            save_button(ui, t, save_key, acts);
            tool_sep(ui);
            match t.sub {
                DesignSub::Fields => field_buttons(ui, t),
                DesignSub::Part(k) => part_buttons(ui, t, k),
                DesignSub::Options | DesignSub::Comment | DesignSub::SqlPreview => {}
            }
            ui.label("Table name:");
            ui.add(egui::TextEdit::singleline(&mut t.name).desired_width(180.0));
            if t.loading || t.saving {
                ui.spinner();
            }
        });
    });
}

fn save_button(ui: &mut Ui, t: &mut DesignTab, save_key: bool, acts: &mut Vec<Act>) {
    let stmts = t.statements();
    let clicked = tool_btn(ui, Some(Icon::Save), "Save", !stmts.is_empty() && !t.saving).on_hover_text("Ctrl+S").clicked();
    if !(clicked || (save_key && !stmts.is_empty())) {
        return;
    }
    if t.name.trim().is_empty() {
        acts.push(Act::Alert("Save table".into(), "Please enter a table name first.".into()));
        return;
    }
    t.saving = true;
    t.error = None;
    let purpose = Purpose::SaveDesign { new_name: t.name.trim().to_string() };
    acts.push(Act::Send(t.cid, Req::Exec { tab: t.id, db: t.db.clone(), stmts, purpose }));
}

fn field_buttons(ui: &mut Ui, t: &mut DesignTab) {
    if tool_btn(ui, Some(Icon::Add), "Add Field", true).clicked() {
        t.add_field();
    }
    if tool_btn(ui, None, "Insert Field", true).clicked() {
        t.insert_field();
    }
    if tool_btn(ui, Some(Icon::Delete), "Delete Field", t.sel.is_some()).clicked() {
        t.delete_field();
    }
    if let (true, Some(i)) = (tool_btn(ui, Some(Icon::Key), "Primary Key", t.sel.is_some()).clicked(), t.sel) {
        t.toggle_key(i);
    }
    tool_sep(ui);
    if tool_btn(ui, None, "Move Up", t.can_move(-1)).clicked() {
        t.move_field(-1);
    }
    if tool_btn(ui, None, "Move Down", t.can_move(1)).clicked() {
        t.move_field(1);
    }
    tool_sep(ui);
}

/// Add / Delete for the Indexes … Triggers tabs.
fn part_buttons(ui: &mut Ui, t: &mut DesignTab, k: PartKind) {
    let ready = !t.parts.loading;
    if tool_btn(ui, Some(Icon::Add), &format!("Add {}", k.noun()), ready).clicked() {
        t.parts.add(k);
    }
    if tool_btn(ui, Some(Icon::Delete), &format!("Delete {}", k.noun()), ready && t.parts.sel(k).is_some()).clicked() {
        t.parts.delete(k);
    }
    tool_sep(ui);
}
