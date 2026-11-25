//! Dropdown and context menus.

use eframe::egui::{self, vec2, Response, Ui};

use crate::app::Act;

/// Style for every dropdown and context menu: roomier than egui's default so the
/// hover highlight doesn't hug the text.
pub fn menu_style(style: &mut egui::Style) {
    egui::containers::menu::menu_style(style);
    style.spacing.button_padding = vec2(12.0, 5.0);
    style.spacing.menu_margin = egui::Margin::same(6);
}

pub fn menu_config() -> egui::containers::menu::MenuConfig {
    egui::containers::menu::MenuConfig::new().style(menu_style)
}

/// Right-click menu on `resp`, using [`menu_style`].
pub fn context_menu(resp: &Response, add: impl FnOnce(&mut Ui)) {
    egui::Popup::context_menu(resp).style(menu_style).show(add);
}

/// A menu entry that pushes `act` and closes the menu.
pub fn menu_item(ui: &mut Ui, acts: &mut Vec<Act>, label: &str, act: Act) {
    if ui.button(label).clicked() {
        acts.push(act);
        ui.close();
    }
}
