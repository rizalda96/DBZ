//! SQL Server's panel under the Triggers grid: Definition (the statements in a code
//! editor, or the CLR method) and Advanced (EXECUTE AS, encryption, replication, append).

use eframe::egui::{self, RichText, Ui};

use super::form_rows::{page_tabs, panel_frame, FIELD_WIDTH};
use crate::domain::TriggerDef;
use crate::sql::EXECUTE_AS;
use crate::ui::theme::pal;
use crate::ui::widgets::code_editor;

const MIN_LINES: usize = 10;

#[derive(Clone, Copy, Default, PartialEq, Eq)]
enum Page {
    #[default]
    Definition,
    Advanced,
}

pub(super) fn panel(ui: &mut Ui, id: u64, trigger: Option<&mut TriggerDef>) {
    egui::Panel::bottom(("dtrigger", id)).resizable(true).default_size(240.0).frame(panel_frame()).show(ui, |ui| {
        let Some(tr) = trigger else {
            ui.label(RichText::new("Select a trigger to edit its definition.").color(pal().muted));
            return;
        };
        match page_tabs(ui, ("dtrigger_page", id), &[(Page::Definition, "Definition"), (Page::Advanced, "Advanced")]) {
            Page::Definition => definition(ui, id, tr),
            Page::Advanced => advanced(ui, id, tr),
        }
    });
}

fn definition(ui: &mut Ui, id: u64, tr: &mut TriggerDef) {
    if tr.external {
        ui.horizontal(|ui| {
            ui.label("External Name:");
            ui.add(egui::TextEdit::singleline(&mut tr.body).desired_width(FIELD_WIDTH).hint_text("[assembly].[class].[method]"));
        });
        return;
    }
    if tr.encrypted && tr.orig_name.is_some() && tr.body.trim().is_empty() {
        ui.label(RichText::new("The server keeps this trigger encrypted, so its statements can't be shown. Changing it needs them typed in again.").color(pal().muted));
    }
    egui::Frame::new().fill(pal().surface).show(ui, |ui| {
        egui::ScrollArea::both().id_salt(("dtrigger_def", id)).auto_shrink(false).show(ui, |ui| {
            code_editor(ui, ("dtrigger_sql", id), &mut tr.body, MIN_LINES);
        });
    });
}

fn advanced(ui: &mut Ui, id: u64, tr: &mut TriggerDef) {
    egui::Grid::new(("dtrigger_adv", id)).num_columns(2).spacing([24.0, 8.0]).min_col_width(140.0).show(ui, |ui| {
        ui.label("Execute As:");
        execute_as(ui, id, &mut tr.execute_as);
        ui.end_row();
        if tr.execute_as == "USER" {
            ui.label("User:");
            ui.add(egui::TextEdit::singleline(&mut tr.execute_as_user).desired_width(FIELD_WIDTH).hint_text("database user name"));
            ui.end_row();
        }
        // CLR code isn't text, so there is nothing to encrypt.
        ui.add_enabled(!tr.external, egui::Checkbox::new(&mut tr.encrypted, "Encrypted"));
        ui.end_row();
        ui.checkbox(&mut tr.not_for_replication, "Not for Replication");
        ui.end_row();
        ui.checkbox(&mut tr.with_append, "With Append").on_hover_text("Only for AFTER triggers on databases in compatibility level 65.");
        ui.end_row();
        ui.label("Definition Type:");
        egui::ComboBox::from_id_salt(("dtrigger_kind", id)).selected_text(if tr.external { "External (CLR)" } else { "SQL Statements" }).width(FIELD_WIDTH).show_ui(ui, |ui| {
            ui.selectable_value(&mut tr.external, false, "SQL Statements");
            ui.selectable_value(&mut tr.external, true, "External (CLR)");
        });
        ui.end_row();
    });
    if tr.external {
        tr.encrypted = false;
    }
}

/// A blank entry (the server's default, the caller) and the EXECUTE AS choices.
fn execute_as(ui: &mut Ui, id: u64, value: &mut String) {
    egui::ComboBox::from_id_salt(("dtrigger_exec_as", id)).selected_text(title(value)).width(FIELD_WIDTH).show_ui(ui, |ui| {
        ui.selectable_value(value, String::new(), " ");
        for who in EXECUTE_AS {
            ui.selectable_value(value, who.to_string(), title(who));
        }
    });
}

/// "OWNER" → "Owner".
fn title(word: &str) -> String {
    let lower = word.to_lowercase();
    let mut chars = lower.chars();
    chars.next().map(|c| c.to_uppercase().chain(chars).collect()).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::title;

    #[test]
    fn titles_execute_as_words() {
        assert_eq!(title("CALLER"), "Caller");
        assert_eq!(title(""), "");
    }
}
