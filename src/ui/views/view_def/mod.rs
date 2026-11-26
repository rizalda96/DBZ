//! The Design View tab: the view's query in the SQL editor. Preview runs the query to
//! show its rows; Run saves it as the view's new definition.

mod state;

pub use state::ViewTab;

use eframe::egui::{self, Ui};

use super::query::{editor_with_results, start_run};
use crate::app::Act;
use crate::sql::beautify;
use crate::ui::icons::Icon;
use crate::ui::theme::pal;
use crate::ui::widgets::{tool_btn, tool_sep, toolbar_frame};

pub fn view_def_view(ui: &mut Ui, t: &mut ViewTab, acts: &mut Vec<Act>) {
    let (save_key, preview_key) = ui.input_mut(|i| {
        let save = i.consume_key(egui::Modifiers::COMMAND, egui::Key::S);
        (save, i.consume_key(egui::Modifiers::COMMAND, egui::Key::R) || i.consume_key(egui::Modifiers::NONE, egui::Key::F9))
    });
    toolbar(ui, t, (save_key, preview_key), acts);
    if let Some(e) = &t.error {
        ui.colored_label(pal().err, format!("  {}", e));
    }
    editor_with_results(ui, &mut t.query);
}

/// Run (save the view), Preview (run the query), Explain and Beautify SQL.
fn toolbar(ui: &mut Ui, t: &mut ViewTab, (save_key, preview_key): (bool, bool), acts: &mut Vec<Act>) {
    toolbar_frame().show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 2.0;
            let idle = !t.saving && !t.query.running;
            let can_run = idle && !t.query.sql.trim().is_empty();
            let run = tool_btn(ui, Some(Icon::Run), "Run", can_run).on_hover_text("Save this query as the view's definition (Ctrl+S)").clicked();
            if (run || save_key) && can_run {
                acts.push(Act::RunView(t.id));
            }
            tool_sep(ui);
            let preview = tool_btn(ui, Some(Icon::View), "Preview", idle).on_hover_text("Show the query's rows without saving (Ctrl+R)").clicked() || preview_key;
            let explain = tool_btn(ui, None, "Explain", idle).clicked();
            if (preview || explain) && idle {
                start_run(&mut t.query, explain, acts);
            }
            tool_sep(ui);
            if tool_btn(ui, None, "Beautify SQL", true).clicked() {
                t.query.sql = beautify(&t.query.sql);
                t.query.dirty = true;
            }
            if t.saving || t.query.running {
                ui.spinner();
            }
        });
    });
}
