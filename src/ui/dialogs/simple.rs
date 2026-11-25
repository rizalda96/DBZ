//! Small dialogs: alert, confirm, new database, save query, rename, about.

use eframe::egui::{self, vec2, Color32, FontId, RichText, Sense};

use crate::ui::widgets::{buttons, win, Choice};
use crate::app::Act;
use crate::domain::DbKind;
use crate::sql::{dialect_for, Dialect};
use crate::ui::icons;
use crate::ui::theme::WARNING;
use crate::worker::{Purpose, Req};

/// Message box with Copy and OK; returns false once closed.
pub(super) fn alert(ctx: &egui::Context, title: &str, msg: &str) -> bool {
    let mut open = true;
    win(title).show(ctx, |ui| {
        ui.set_max_width(460.0);
        egui::ScrollArea::vertical().max_height(300.0).show(ui, |ui| {
            ui.add(egui::Label::new(msg).wrap().selectable(true));
        });
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            if ui.button("Copy").clicked() {
                ui.ctx().copy_text(msg.to_string());
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let key = ui.input(|i| i.key_pressed(egui::Key::Enter) || i.key_pressed(egui::Key::Escape));
                open = !(ui.button("   OK   ").clicked() || key);
            });
        });
    });
    open
}

pub(super) fn confirm(ctx: &egui::Context, title: &str, msg: &str) -> Choice {
    let mut choice = Choice::None;
    win(title).show(ctx, |ui| {
        ui.set_max_width(420.0);
        ui.horizontal(|ui| {
            let (r, _) = ui.allocate_exact_size(vec2(32.0, 32.0), Sense::hover());
            ui.painter().circle_filled(r.center(), 14.0, WARNING);
            ui.painter().text(r.center(), egui::Align2::CENTER_CENTER, "!", FontId::proportional(18.0), Color32::WHITE);
            ui.add(egui::Label::new(msg).wrap());
        });
        choice = buttons(ui, "OK", true);
    });
    choice
}

pub(super) fn new_db(ctx: &egui::Context, (cid, kind): (u64, DbKind), name: &mut String, charset: &mut String, acts: &mut Vec<Act>) -> Choice {
    let mut choice = Choice::None;
    let charset_label = if kind == DbKind::PostgreSql { "Encoding:" } else { "Character Set:" };
    win("New Database").show(ctx, |ui| {
        egui::Grid::new("newdb").num_columns(2).spacing([10.0, 8.0]).show(ui, |ui| {
            ui.label("Database Name:");
            ui.add(egui::TextEdit::singleline(name).desired_width(240.0));
            ui.end_row();
            if !kind.is_builtin() {
                ui.label(charset_label);
                ui.add(egui::TextEdit::singleline(charset).desired_width(240.0));
                ui.end_row();
            }
        });
        choice = buttons(ui, "OK", !name.trim().is_empty());
    });
    if choice == Choice::Ok {
        // The sample engine has no character sets; quotes, semicolons and spaces are never valid in one.
        let cs = if kind.is_builtin() { String::new() } else { charset.trim().replace(['\'', ';', ' '], "") };
        let sql = dialect_for(kind).create_database(name.trim(), &cs);
        acts.push(Act::Send(cid, Req::Exec { tab: 0, db: String::new(), stmts: vec![sql], purpose: Purpose::Other("database".into()) }));
    }
    choice
}

pub(super) fn save_query(ctx: &egui::Context, tab: u64, name: &mut String, acts: &mut Vec<Act>) -> Choice {
    let mut choice = Choice::None;
    win("Save Query").show(ctx, |ui| {
        ui.label("Query name:");
        ui.add(egui::TextEdit::singleline(name).desired_width(280.0));
        choice = buttons(ui, "Save", !name.trim().is_empty());
    });
    if choice == Choice::Ok {
        acts.push(Act::WriteQuery(tab, name.trim().to_string()));
    }
    choice
}

pub(super) fn rename(ctx: &egui::Context, (cid, dialect): (u64, &dyn Dialect), (db, table): (&str, &str), new: &mut String, acts: &mut Vec<Act>) -> Choice {
    let mut choice = Choice::None;
    win("Rename Table").show(ctx, |ui| {
        ui.label(format!("New name for \"{}\":", table));
        ui.add(egui::TextEdit::singleline(new).desired_width(280.0));
        choice = buttons(ui, "OK", !new.trim().is_empty() && new.trim() != table);
    });
    if choice == Choice::Ok {
        let sql = dialect.rename_table(table, new.trim());
        acts.push(Act::Send(cid, Req::Exec { tab: 0, db: db.to_string(), stmts: vec![sql], purpose: Purpose::Refresh }));
    }
    choice
}

pub(super) fn about(ctx: &egui::Context) -> Choice {
    let mut choice = Choice::None;
    win("About DBZ").show(ctx, |ui| {
        ui.horizontal(|ui| {
            icons::logo(ui, 56.0);
            ui.vertical(|ui| {
                ui.label(RichText::new("DBZ").strong().size(18.0));
                ui.label("Version 0.1 — a database manager written in Rust");
            });
        });
        ui.add_space(6.0);
        ui.label("Connects to MySQL, MariaDB and PostgreSQL, and includes a built-in sample database to try things out.");
        ui.add_space(8.0);
        if ui.button("   OK   ").clicked() {
            choice = Choice::Ok;
        }
    });
    choice
}
