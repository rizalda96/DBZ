//! The File / View / Tools / Window / Help menu bar.

use eframe::egui::{self, Ui};

use super::object_menus::archive_items;
use crate::app::{Act, ObjMode, ObjRef, PaneState, Workspace};
use crate::domain::DbKind;
use crate::ui::theme::ThemePref;
use crate::ui::widgets::{menu_config, menu_item, menu_style};

const SHORTCUTS_HELP: &str = "Ctrl+Q — New query\nCtrl+R or F9 — Run query (or just the selected text)\nCtrl+S — Save (query, data changes, table design)\nCtrl+W — Close tab\nF5 — Refresh\nEnter or F2 — Edit the selected cell\nArrow keys — Move between cells\nCtrl+C — Copy the selected cell";
const VIEW_MODES: [ObjMode; 6] = [ObjMode::Tables, ObjMode::Views, ObjMode::Functions, ObjMode::Users, ObjMode::Queries, ObjMode::Backups];

pub fn menu_bar(ui: &mut Ui, ws: &Workspace, panes: &mut PaneState, theme: ThemePref, acts: &mut Vec<Act>) {
    egui::MenuBar::new().style(menu_style).config(menu_config()).ui(ui, |ui| {
        ui.menu_button("File", |ui| file_menu(ui, ws, acts));
        ui.menu_button("View", |ui| view_menu(ui, ws, panes, theme, acts));
        ui.menu_button("Tools", |ui| tools_menu(ui, ws, acts));
        ui.menu_button("Window", |ui| window_menu(ui, ws, acts));
        ui.menu_button("Help", |ui| {
            menu_item(ui, acts, "Keyboard Shortcuts", Act::Alert("Keyboard Shortcuts".into(), SHORTCUTS_HELP.into()));
            menu_item(ui, acts, "About DBZ", Act::ShowAbout);
        });
    });
}

/// A menu entry that is greyed out when `act` is None.
fn item_if(ui: &mut Ui, acts: &mut Vec<Act>, button: egui::Button, act: Option<Act>) {
    if ui.add_enabled(act.is_some(), button).clicked() {
        acts.extend(act);
        ui.close();
    }
}

fn file_menu(ui: &mut Ui, ws: &Workspace, acts: &mut Vec<Act>) {
    ui.menu_button("New Connection", |ui| {
        for k in DbKind::ALL {
            menu_item(ui, acts, k.label(), Act::NewConn(k));
        }
    });
    let new_query = match ws.query_target() {
        Some(cid) => Act::NewQuery(cid, ws.sel.db.clone().unwrap_or_default(), String::new()),
        None => Act::Alert("New Query".into(), "Create a connection first.".into()),
    };
    item_if(ui, acts, egui::Button::new("New Query").shortcut_text("Ctrl+Q"), Some(new_query));
    menu_item(ui, acts, "Open SQL File…", Act::OpenSqlFile);
    ui.separator();
    item_if(ui, acts, egui::Button::new("Execute SQL File…"), ws.sel.conn_db().map(|(c, d)| Act::RunFile(c, d)));
    ui.separator();
    item_if(ui, acts, egui::Button::new("Close Tab").shortcut_text("Ctrl+W"), (ws.active > 0).then_some(Act::CloseTab(ws.active)));
    if ui.button("Exit").clicked() {
        ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
    }
}

fn view_menu(ui: &mut Ui, ws: &Workspace, panes: &mut PaneState, theme: ThemePref, acts: &mut Vec<Act>) {
    if ui.checkbox(&mut panes.show_info, "Information Pane").clicked() {
        ui.close();
    }
    ui.separator();
    for m in VIEW_MODES {
        if ui.selectable_label(ws.showing(m), m.label()).clicked() {
            acts.push(Act::SetMode(m));
            ui.close();
        }
    }
    ui.separator();
    ui.menu_button("Theme", |ui| {
        for t in ThemePref::ALL {
            if ui.radio(theme == t, t.label()).clicked() {
                acts.push(Act::SetTheme(t));
                ui.close();
            }
        }
    });
    ui.separator();
    if ui.add(egui::Button::new("Refresh").shortcut_text("F5")).clicked() {
        acts.extend(ws.refresh_acts());
        ui.close();
    }
}

fn tools_menu(ui: &mut Ui, ws: &Workspace, acts: &mut Vec<Act>) {
    let target = ws.sel.conn_db();
    let table = if ws.mode == ObjMode::Tables { ws.sel.obj.clone() } else { None };
    let with = |f: &dyn Fn(u64, String) -> Act| target.clone().map(|(c, d)| f(c, d));
    item_if(ui, acts, egui::Button::new("Import Wizard…"), with(&|c, d| Act::Import(c, d, table.clone())));
    let export = target.clone().zip(table.clone()).map(|((cid, db), name)| Act::Export(ObjRef { cid, db, name }));
    item_if(ui, acts, egui::Button::new("Export Wizard…"), export);
    ui.separator();
    item_if(ui, acts, egui::Button::new("Dump SQL File (Structure + Data)…"), with(&|c, d| Act::Dump(c, d, true)));
    item_if(ui, acts, egui::Button::new("Dump SQL File (Structure Only)…"), with(&|c, d| Act::Dump(c, d, false)));
    item_if(ui, acts, egui::Button::new("Execute SQL File…"), with(&|c, d| Act::RunFile(c, d)));
    if let Some((cid, db)) = target.clone().filter(|(c, _)| ws.kind_of(*c).archive_tools().is_some()) {
        ui.separator();
        archive_items(ui, cid, &db, acts);
    }
    ui.separator();
    if ui.add_enabled(target.is_some(), egui::Button::new("New Backup")).clicked() {
        acts.extend(with(&|c, d| Act::NewBackup(c, d)));
        acts.push(Act::SetMode(ObjMode::Backups));
        ui.close();
    }
}

fn window_menu(ui: &mut Ui, ws: &Workspace, acts: &mut Vec<Act>) {
    if ui.selectable_label(ws.active == 0, "Objects").clicked() {
        acts.push(Act::Activate(0));
        ui.close();
    }
    for (i, t) in ws.tabs.iter().enumerate() {
        if ui.selectable_label(ws.active == i + 1, t.title(&ws.conns)).clicked() {
            acts.push(Act::Activate(i + 1));
            ui.close();
        }
    }
    ui.separator();
    if ui.button("Close All Tabs").clicked() {
        acts.extend((1..=ws.tabs.len()).rev().map(Act::CloseTab));
        ui.close();
    }
}
