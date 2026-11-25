//! The big-button ribbon under the menu.

use eframe::egui::{self, Ui};

use crate::app::{Act, ObjMode, ObjRef, Workspace};
use crate::domain::DbKind;
use crate::ui::icons::Icon;
use crate::ui::widgets::{menu_style, ribbon_btn, ribbon_sep};

const OBJECT_MODES: [(ObjMode, &str); 4] = [(ObjMode::Tables, "Table"), (ObjMode::Views, "View"), (ObjMode::Functions, "Function"), (ObjMode::Users, "User")];
const FILE_MODES: [(ObjMode, &str); 2] = [(ObjMode::Queries, "Query"), (ObjMode::Backups, "Backup")];

pub fn ribbon(ui: &mut Ui, ws: &Workspace, acts: &mut Vec<Act>) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 2.0;
        connection_button(ui, acts);
        let qcid = ws.query_target();
        if let (true, Some(cid)) = (ribbon_btn(ui, Icon::NewQuery, "New Query", false, qcid.is_some()).clicked(), qcid) {
            acts.push(Act::NewQuery(cid, ws.sel.db.clone().unwrap_or_default(), String::new()));
        }
        ribbon_sep(ui);
        mode_buttons(ui, ws, &OBJECT_MODES, acts);
        ribbon_sep(ui);
        mode_buttons(ui, ws, &FILE_MODES, acts);
        ribbon_sep(ui);
        transfer_buttons(ui, ws, acts);
    });
}

fn connection_button(ui: &mut Ui, acts: &mut Vec<Act>) {
    let r = ribbon_btn(ui, Icon::Connection, "Connection", false, true);
    egui::Popup::menu(&r).style(menu_style).show(|ui| {
        ui.set_min_width(200.0);
        for k in DbKind::ALL {
            if ui.button(k.label()).clicked() {
                acts.push(Act::NewConn(k));
                ui.close();
            }
        }
    });
}

fn mode_buttons(ui: &mut Ui, ws: &Workspace, modes: &[(ObjMode, &str)], acts: &mut Vec<Act>) {
    for (m, label) in modes {
        if ribbon_btn(ui, m.icon(), label, ws.showing(*m), true).clicked() {
            acts.push(Act::SetMode(*m));
        }
    }
}

fn transfer_buttons(ui: &mut Ui, ws: &Workspace, acts: &mut Vec<Act>) {
    let target = ws.sel.conn_db();
    let import = ribbon_btn(ui, Icon::Import, "Import", false, target.is_some()).clicked();
    if let (true, Some((cid, db))) = (import, target.clone()) {
        let table = if ws.mode == ObjMode::Tables { ws.sel.obj.clone() } else { None };
        acts.push(Act::Import(cid, db, table));
    }
    let export_target = target.zip(ws.sel.obj.clone()).filter(|_| ws.mode == ObjMode::Tables);
    let export = ribbon_btn(ui, Icon::Export, "Export", false, export_target.is_some()).clicked();
    if let (true, Some(((cid, db), name))) = (export, export_target) {
        acts.push(Act::Export(ObjRef { cid, db, name }));
    }
}
