//! Pieces shared by the designer's property panels under the grids (index, foreign
//! key, unique, trigger, rule): the panel frame, a long-text panel, a searchable
//! drop-down and the DEFERRABLE / INITIALLY DEFERRED pair.

use eframe::egui::{self, RichText, Stroke, Ui};

use crate::ui::theme::pal;
use crate::ui::widgets::search_combo;

/// Width of a panel's value column.
pub(super) const FIELD_WIDTH: f32 = 370.0;

/// A searchable drop-down led by an empty entry, panel-wide; true when a value was picked.
pub(super) fn combo(ui: &mut Ui, salt: impl std::hash::Hash + std::fmt::Debug, value: &mut String, options: &[String]) -> bool {
    search_combo(ui, (salt, FIELD_WIDTH), value, options)
}

/// Wording of a [`body_panel`].
pub(super) struct PanelText {
    pub label: &'static str,
    pub hint: &'static str,
    /// Shown while no row is selected.
    pub empty: &'static str,
}

/// A resizable box under the grid for the selected row's long text.
pub(super) fn body_panel(ui: &mut Ui, salt: (&'static str, u64), text: &PanelText, body: Option<&mut String>) {
    egui::Panel::bottom(salt).resizable(true).default_size(120.0).frame(panel_frame()).show(ui, |ui| {
        let Some(body) = body else {
            ui.label(RichText::new(text.empty).color(pal().muted));
            return;
        };
        ui.label(format!("{}:", text.label));
        ui.add_sized([ui.available_width(), ui.available_height().max(40.0)], egui::TextEdit::multiline(body).hint_text(text.hint).font(egui::TextStyle::Monospace));
    });
}

/// The look of the property panels under the grids.
pub(super) fn panel_frame() -> egui::Frame {
    egui::Frame::new().fill(pal().toolbar_bg).inner_margin(8).stroke(Stroke::new(1.0, pal().border))
}

/// "Deferrable:" and "Deferred:" rows of a `Grid`; Deferred is greyed out (and kept
/// off) while the constraint isn't deferrable.
pub(super) fn deferrable_rows(ui: &mut Ui, salt: (&'static str, u64), deferrable: &mut bool, deferred: &mut bool) {
    ui.label("Deferrable:");
    let shown = if *deferrable { "DEFERRABLE" } else { "NOT DEFERRABLE" };
    egui::ComboBox::from_id_salt((salt, "deferrable")).selected_text(shown).width(FIELD_WIDTH).show_ui(ui, |ui| {
        ui.selectable_value(deferrable, false, "NOT DEFERRABLE");
        ui.selectable_value(deferrable, true, "DEFERRABLE");
    });
    ui.end_row();
    let on = *deferrable;
    ui.add_enabled(on, egui::Label::new("Deferred:"));
    ui.add_enabled_ui(on, |ui| {
        let shown = if *deferred { "INITIALLY DEFERRED" } else { "INITIALLY IMMEDIATE" };
        egui::ComboBox::from_id_salt((salt, "deferred")).selected_text(shown).width(FIELD_WIDTH).show_ui(ui, |ui| {
            ui.selectable_value(deferred, false, "INITIALLY IMMEDIATE");
            ui.selectable_value(deferred, true, "INITIALLY DEFERRED");
        });
    });
    ui.end_row();
    // INITIALLY DEFERRED means nothing without DEFERRABLE; keeping it off stops the
    // constraint from looking changed.
    if !on {
        *deferred = false;
    }
}
