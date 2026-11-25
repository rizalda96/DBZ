//! The table designer tab: fields, indexes, keys, checks, triggers, options, table
//! comment and SQL preview.

mod cell_window;
mod fk_view;
mod form_rows;
mod grid;
mod index_cols_state;
mod index_cols_view;
mod index_props_view;
mod parts_cells;
mod parts_grid;
mod parts_state;
mod parts_view;
mod ref_state;
mod state;
mod tick_state;
mod tick_view;
mod toolbar;
mod trigger_view;
mod unique_view;

pub use state::DesignTab;

use state::DesignSub;

use eframe::egui::{self, FontId, RichText, Stroke, Ui};

use crate::app::Act;
use crate::ui::theme::pal;
use crate::ui::widgets::sql_layouter;
use crate::worker::Req;

pub fn design_view(ui: &mut Ui, t: &mut DesignTab, acts: &mut Vec<Act>) {
    let save_key = ui.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::S));
    toolbar::show(ui, t, save_key, acts);
    sub_tabs(ui, t);
    if let Some(e) = &t.error {
        ui.colored_label(pal().err, format!("  {}", e));
    }
    match t.sub {
        DesignSub::Fields => fields(ui, t),
        DesignSub::Part(k) => parts_view::list(ui, t, k),
        DesignSub::Options => parts_view::options(ui, t),
        DesignSub::Comment => comment(ui, t),
        DesignSub::SqlPreview => sql_preview(ui, t),
    }
    for (schema, table) in t.ref_columns.take_requests() {
        acts.push(Act::Send(t.cid, Req::RefColumns { db: t.db.clone(), schema, table }));
    }
}

fn sub_tabs(ui: &mut Ui, t: &mut DesignTab) {
    egui::Frame::new().fill(pal().header_bg).inner_margin(egui::Margin::symmetric(6, 2)).show(ui, |ui| {
        ui.horizontal(|ui| {
            let caps = t.dialect().designer();
            for sub in DesignSub::all().filter(|s| s.offered(caps)) {
                if ui.selectable_label(t.sub == sub, sub.label()).clicked() {
                    t.sub = sub;
                }
            }
        });
    });
}

fn comment(ui: &mut Ui, t: &mut DesignTab) {
    ui.add_space(6.0);
    ui.label("  Table comment:");
    ui.add_sized([ui.available_width() - 12.0, 120.0], egui::TextEdit::multiline(&mut t.comment));
}

fn sql_preview(ui: &mut Ui, t: &DesignTab) {
    let mut s = t.statements().iter().map(|s| format!("{};", s)).collect::<Vec<_>>().join("\n\n");
    if s.is_empty() {
        s = "-- No changes".into();
    }
    let mut layouter = sql_layouter;
    egui::ScrollArea::both().auto_shrink(false).show(ui, |ui| {
        ui.add(egui::TextEdit::multiline(&mut s).font(FontId::monospace(13.0)).desired_width(f32::INFINITY).interactive(true).frame(egui::Frame::NONE).layouter(&mut layouter));
    });
}

fn fields(ui: &mut Ui, t: &mut DesignTab) {
    let frame = egui::Frame::new().fill(pal().toolbar_bg).inner_margin(8).stroke(Stroke::new(1.0, pal().border));
    egui::Panel::bottom(("dprops", t.id)).resizable(true).default_size(120.0).frame(frame).show(ui, |ui| properties(ui, t));
    egui::CentralPanel::default().frame(egui::Frame::new().fill(pal().surface)).show(ui, |ui| grid::show(ui, t));
}

/// Default, auto increment and comment of the selected field.
fn properties(ui: &mut Ui, t: &mut DesignTab) {
    let Some(f) = t.sel.and_then(|i| t.fields.get_mut(i)) else {
        ui.label(RichText::new("Select a field to see its properties.").color(pal().muted));
        return;
    };
    egui::Grid::new(("fprops", t.id)).num_columns(2).spacing([12.0, 6.0]).show(ui, |ui| {
        ui.label("Default:");
        ui.add(egui::TextEdit::singleline(&mut f.default).hint_text("(none)").desired_width(260.0));
        ui.end_row();
        ui.label("Auto Increment:");
        ui.checkbox(&mut f.auto_inc, "");
        ui.end_row();
        ui.label("Comment:");
        ui.add(egui::TextEdit::singleline(&mut f.comment).desired_width(260.0));
        ui.end_row();
    });
}
