//! The table data tab: a filterable, sortable, editable grid with paging.

mod cell_menu;
mod footer;
mod grid;
mod state;
mod toolbar;

pub use state::{DataTab, RowState};

use eframe::egui::{self, Ui};

use crate::app::Act;
use crate::ui::theme::pal;

pub fn data_view(ui: &mut Ui, t: &mut DataTab, acts: &mut Vec<Act>) {
    let editable = !t.is_view;
    let save_key = ui.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::S));
    toolbar::show(ui, t, editable, acts);
    footer::show(ui, t, editable, save_key, acts);
    egui::CentralPanel::default().frame(egui::Frame::new().fill(pal().surface)).show(ui, |ui| body(ui, t, editable, acts));
}

fn body(ui: &mut Ui, t: &mut DataTab, editable: bool, acts: &mut Vec<Act>) {
    if let Some(e) = &t.error {
        ui.add_space(10.0);
        ui.colored_label(pal().err, format!("  {}", e));
        return;
    }
    if t.columns.is_empty() {
        ui.add_space(10.0);
        if t.loading {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label("Loading data…");
            });
        }
        return;
    }
    grid::show(ui, t, editable, acts);
}

/// Reloads the current page.
fn reload(t: &mut DataTab, acts: &mut Vec<Act>) {
    acts.push(Act::Send(t.cid, t.reload_req()));
}
