//! Rows shared by the designer's property panels (index, foreign key, unique): a
//! searchable drop-down and the DEFERRABLE / INITIALLY DEFERRED pair.

use eframe::egui::{self, Ui};

use crate::ui::widgets::{forget_search, search_list};

/// Width of a panel's value column.
pub(super) const FIELD_WIDTH: f32 = 370.0;

/// A searchable drop-down led by an empty entry; true when a value was picked.
pub(super) fn combo(ui: &mut Ui, salt: impl std::hash::Hash + std::fmt::Debug, value: &mut String, options: &[String]) -> bool {
    // Its own search text, next to the combo's id.
    let search = ui.scope_id().with(("combo_search", &salt));
    let combo = egui::ComboBox::from_id_salt(salt).selected_text(value.clone()).width(FIELD_WIDTH);
    let entries = std::iter::once("").chain(options.iter().map(String::as_str));
    let shown = combo.close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside).show_ui(ui, |ui| search_list(ui, search, value, entries));
    let Some(picked) = shown.inner else {
        forget_search(ui, search);
        return false;
    };
    let Some(p) = picked else { return false };
    *value = p;
    true
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
