//! The materialized view designer tab (PostgreSQL): toolbar, then the Definition,
//! Advanced, Comment and SQL Preview pages.

mod state;

pub use state::MatViewTab;

use eframe::egui::{self, Ui};

use super::query::{editor_with_results, start_run};
use crate::app::Act;
use crate::sql::beautify;
use crate::ui::icons::Icon;
use crate::ui::theme::pal;
use crate::ui::widgets::{search_combo, sql_preview, tool_btn, tool_sep, toolbar_frame};
use state::MatViewSub;

const FIELD_WIDTH: f32 = 300.0;

pub fn matview_view(ui: &mut Ui, t: &mut MatViewTab, acts: &mut Vec<Act>) {
    let (save_key, run_key) = ui.input_mut(|i| {
        let save = i.consume_key(egui::Modifiers::COMMAND, egui::Key::S);
        (save, i.consume_key(egui::Modifiers::COMMAND, egui::Key::R) || i.consume_key(egui::Modifiers::NONE, egui::Key::F9))
    });
    toolbar(ui, t, (save_key, run_key), acts);
    sub_tabs(ui, t);
    if let Some(e) = &t.error {
        ui.colored_label(pal().err, format!("  {}", e));
    }
    match t.sub {
        MatViewSub::Definition => editor_with_results(ui, &mut t.query),
        MatViewSub::Advanced => advanced(ui, t),
        MatViewSub::Comment => comment(ui, t),
        MatViewSub::SqlPreview => sql_preview(ui, &t.statements(), "-- No changes"),
    }
}

/// Save, Preview (run the SELECT), Explain and Beautify SQL.
fn toolbar(ui: &mut Ui, t: &mut MatViewTab, (save_key, run_key): (bool, bool), acts: &mut Vec<Act>) {
    toolbar_frame().show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 2.0;
            let can_save = !t.saving && !t.statements().is_empty();
            if (tool_btn(ui, Some(Icon::Save), "Save", can_save).on_hover_text("Ctrl+S").clicked() || save_key) && can_save {
                acts.push(Act::SaveMatView(t.id));
            }
            tool_sep(ui);
            let idle = !t.query.running;
            let preview = tool_btn(ui, Some(Icon::Run), "Preview", idle).on_hover_text("Ctrl+R").clicked() || run_key;
            let explain = tool_btn(ui, None, "Explain", idle).clicked();
            if (preview || explain) && idle {
                t.sub = MatViewSub::Definition;
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

fn sub_tabs(ui: &mut Ui, t: &mut MatViewTab) {
    egui::Frame::new().fill(pal().header_bg).inner_margin(egui::Margin::symmetric(6, 2)).show(ui, |ui| {
        ui.horizontal(|ui| {
            for sub in MatViewSub::ALL {
                ui.selectable_value(&mut t.sub, sub, sub.label());
            }
        });
    });
}

/// Owner, tablespace, fill factor and WITH DATA.
fn advanced(ui: &mut Ui, t: &mut MatViewTab) {
    let id = t.id;
    let (def, choices) = (&mut t.def, &t.choices);
    egui::Frame::new().inner_margin(12).show(ui, |ui| {
        egui::Grid::new(("mv_adv", id)).num_columns(2).spacing([24.0, 8.0]).min_col_width(140.0).show(ui, |ui| {
            ui.label("Owner:");
            search_combo(ui, (("mv_owner", id), FIELD_WIDTH), &mut def.owner, &choices.roles);
            ui.end_row();
            ui.label("Tablespace:");
            search_combo(ui, (("mv_ts", id), FIELD_WIDTH), &mut def.tablespace, &choices.tablespaces);
            ui.end_row();
            ui.label("Fill Factor:");
            ui.add(egui::TextEdit::singleline(&mut def.fill_factor).desired_width(FIELD_WIDTH).hint_text("10 – 100"));
            ui.end_row();
        });
        ui.add_space(4.0);
        ui.checkbox(&mut def.with_data, "With Data");
    });
}

fn comment(ui: &mut Ui, t: &mut MatViewTab) {
    ui.add_space(6.0);
    ui.label("  Comment:");
    ui.add_sized([ui.available_width() - 12.0, 120.0], egui::TextEdit::multiline(&mut t.def.comment));
}
