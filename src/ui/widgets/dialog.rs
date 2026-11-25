//! Pieces shared by every dialog window.

use eframe::egui::{self, RichText, Ui};

/// A centred, fixed-size window.
pub fn win(title: &str) -> egui::Window<'static> {
    egui::Window::new(title.to_string()).collapsible(false).resizable(false).anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
}

/// How the user left a dialog this frame.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Choice {
    None,
    Ok,
    Cancel,
}

impl Choice {
    /// Keep the dialog open?
    pub fn keep(self) -> bool {
        self == Choice::None
    }
}

/// Right-aligned OK / Cancel; Escape cancels.
pub fn buttons(ui: &mut Ui, ok_label: &str, ok_enabled: bool) -> Choice {
    let mut choice = Choice::None;
    ui.add_space(8.0);
    // The row keeps the right-to-left layout one line tall; on its own it takes all
    // free height and stretches the window.
    ui.horizontal(|ui| {
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.button("Cancel").clicked() || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                choice = Choice::Cancel;
            }
            if ui.add_enabled(ok_enabled, egui::Button::new(RichText::new(format!("  {}  ", ok_label)).strong())).clicked() {
                choice = Choice::Ok;
            }
        });
    });
    choice
}

/// A path box with a "…" button that opens a file picker.
pub fn file_row(ui: &mut Ui, path: &mut String, hint: &str, pick: impl FnOnce() -> Option<std::path::PathBuf>) {
    ui.horizontal(|ui| {
        ui.add(egui::TextEdit::singleline(path).desired_width(340.0).hint_text(hint));
        let picked = if ui.button("…").clicked() { pick() } else { None };
        if let Some(p) = picked {
            *path = p.to_string_lossy().to_string();
        }
    });
}
