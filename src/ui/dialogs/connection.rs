//! The New / Edit Connection window.

use eframe::egui::{self, vec2, RichText, Sense, Ui};

use crate::ui::widgets::win;
use crate::app::{Act, ConnDialog};
use crate::domain::{ConnConfig, DbKind};
use crate::ui::icons::{self, Icon};
use crate::ui::theme::{kind_color, pal};

const PAGES: [&str; 2] = ["General", "Advanced"];

pub(super) fn show(ctx: &egui::Context, d: &mut ConnDialog, acts: &mut Vec<Act>) {
    let title = if d.editing.is_some() { "Edit Connection" } else { "New Connection" };
    win(title).show(ctx, |ui| {
        ui.set_width(500.0);
        ui.horizontal(|ui| {
            for (i, l) in PAGES.iter().enumerate() {
                if ui.selectable_label(d.page == i, *l).clicked() {
                    d.page = i;
                }
            }
        });
        ui.separator();
        kind_header(ui, d.cfg.kind);
        ui.add_space(8.0);
        egui::Grid::new("conn_grid").num_columns(2).spacing([12.0, 8.0]).min_col_width(130.0).show(ui, |ui| {
            if d.page == 0 {
                general_page(ui, &mut d.cfg);
            } else {
                advanced_page(ui, &mut d.cfg);
            }
        });
        if !d.cfg.save_password {
            ui.label(RichText::new("It will be kept only until the app closes.").color(pal().muted).size(12.0));
        }
        ui.add_space(8.0);
        test_result(ui, d);
        ui.separator();
        footer(ui, d, acts);
    });
}

fn kind_header(ui: &mut Ui, kind: DbKind) {
    ui.horizontal(|ui| {
        let (r, _) = ui.allocate_exact_size(vec2(44.0, 44.0), Sense::hover());
        icons::paint(ui.painter(), r, Icon::Conn { color: kind_color(kind), open: true });
        ui.add_space(8.0);
        ui.label(RichText::new(kind.label()).size(16.0).strong());
    });
}

fn general_page(ui: &mut Ui, cfg: &mut ConnConfig) {
    ui.label("Connection Name:");
    ui.add(egui::TextEdit::singleline(&mut cfg.name).desired_width(300.0).hint_text("e.g. Local MySQL"));
    ui.end_row();
    ui.label("Type:");
    let mut kind = cfg.kind;
    egui::ComboBox::from_id_salt("kind").selected_text(kind.label()).width(300.0).show_ui(ui, |ui| {
        for k in DbKind::ALL {
            ui.selectable_value(&mut kind, k, k.label());
        }
    });
    change_kind(cfg, kind);
    ui.end_row();
    server_fields(ui, cfg);
}

/// Switching kind updates port, user and host when they still hold the old defaults.
fn change_kind(cfg: &mut ConnConfig, kind: DbKind) {
    if kind == cfg.kind {
        return;
    }
    if cfg.port == cfg.kind.default_port() || cfg.port == 0 {
        cfg.port = kind.default_port();
    }
    if cfg.user.is_empty() || cfg.user == cfg.kind.default_user() {
        cfg.user = kind.default_user().into();
    }
    if cfg.host.is_empty() {
        cfg.host = "localhost".into();
    }
    cfg.kind = kind;
}

fn server_fields(ui: &mut Ui, cfg: &mut ConnConfig) {
    ui.label("Host:");
    ui.add(egui::TextEdit::singleline(&mut cfg.host).desired_width(300.0));
    ui.end_row();
    ui.label("Port:");
    number_field(ui, &mut cfg.port);
    ui.end_row();
    ui.label("User Name:");
    ui.add(egui::TextEdit::singleline(&mut cfg.user).desired_width(300.0));
    ui.end_row();
    ui.label("Password:");
    ui.add(egui::TextEdit::singleline(&mut cfg.password).password(true).desired_width(300.0));
    ui.end_row();
    ui.label("");
    ui.checkbox(&mut cfg.save_password, "Save password");
    ui.end_row();
}

/// A small text box for a number; text that doesn't parse leaves the value unchanged.
fn number_field<T: std::str::FromStr + ToString + Copy>(ui: &mut Ui, value: &mut T) {
    let mut text = value.to_string();
    if ui.add(egui::TextEdit::singleline(&mut text).desired_width(80.0)).changed() {
        *value = text.trim().parse().unwrap_or(*value);
    }
}

fn advanced_page(ui: &mut Ui, cfg: &mut ConnConfig) {
    ui.label("Initial Database:");
    let hint = if cfg.kind == DbKind::PostgreSql { "postgres" } else { "(none)" };
    ui.add(egui::TextEdit::singleline(&mut cfg.initial_db).desired_width(300.0).hint_text(hint));
    ui.end_row();
    ui.label("Connect Timeout (s):");
    number_field(ui, &mut cfg.connect_timeout);
    ui.end_row();
}

fn test_result(ui: &mut Ui, d: &ConnDialog) {
    match &d.test {
        Some(Ok(m)) => {
            ui.colored_label(pal().ok, format!("Success: {}", m));
        }
        Some(Err(e)) => {
            ui.add(egui::Label::new(RichText::new(format!("Failed: {}", e)).color(pal().err)).wrap());
        }
        None if d.test_id.is_some() => {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label("Testing…");
            });
        }
        None => {}
    }
}

fn footer(ui: &mut Ui, d: &mut ConnDialog, acts: &mut Vec<Act>) {
    ui.horizontal(|ui| {
        if ui.button("Test Connection").clicked() {
            acts.push(Act::TestConn(d.cfg.clone()));
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.button("Cancel").clicked() {
                acts.push(Act::CloseConnDialog);
            }
            if ui.button(RichText::new("   OK   ").strong()).clicked() {
                acts.push(Act::SaveConn { editing: d.editing, cfg: d.cfg.clone() });
            }
        });
    });
}
