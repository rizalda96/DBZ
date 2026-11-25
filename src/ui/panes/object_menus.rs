//! Right-click menus for databases and objects, and what double-click opens.

use eframe::egui::Ui;

use crate::app::{Act, ObjMode, ObjRef, Sel};
use crate::ui::widgets::menu_item;

/// What double-clicking an object in `mode` does.
pub(super) fn open_act(mode: ObjMode, obj: ObjRef, is_view: bool) -> Act {
    match mode {
        ObjMode::Tables | ObjMode::Views => Act::OpenTable(obj, is_view),
        ObjMode::Queries => Act::OpenSavedQuery(obj),
        ObjMode::Backups => Act::RestoreBackup(obj),
        _ => Act::Select(Sel { cid: Some(obj.cid), db: Some(obj.db), obj: Some(obj.name) }),
    }
}

pub(super) fn db_menu(ui: &mut Ui, cid: u64, db: &str, expanded: bool, acts: &mut Vec<Act>) {
    let db = db.to_string();
    if expanded {
        menu_item(ui, acts, "Close Database", Act::CloseDb(cid, db.clone()));
    } else {
        menu_item(ui, acts, "Open Database", Act::OpenDb(cid, db.clone()));
    }
    menu_item(ui, acts, "New Database…", Act::NewDb(cid));
    menu_item(ui, acts, "Delete Database", Act::DropDb(cid, db.clone()));
    ui.separator();
    menu_item(ui, acts, "New Query", Act::NewQuery(cid, db.clone(), String::new()));
    menu_item(ui, acts, "New Table", Act::DesignTable(cid, db.clone(), None));
    ui.separator();
    menu_item(ui, acts, "Execute SQL File…", Act::RunFile(cid, db.clone()));
    menu_item(ui, acts, "Dump SQL File (Structure + Data)…", Act::Dump(cid, db.clone(), true));
    menu_item(ui, acts, "Dump SQL File (Structure Only)…", Act::Dump(cid, db.clone(), false));
    menu_item(ui, acts, "Import Wizard…", Act::Import(cid, db.clone(), None));
    menu_item(ui, acts, "New Backup", Act::NewBackup(cid, db.clone()));
    ui.separator();
    menu_item(ui, acts, "Refresh", Act::RefreshDb(cid, db));
}

pub(super) fn object_menu(ui: &mut Ui, mode: ObjMode, obj: &ObjRef, acts: &mut Vec<Act>) {
    match mode {
        ObjMode::Tables => table_menu(ui, obj, acts),
        ObjMode::Views => {
            menu_item(ui, acts, "Open View", Act::OpenTable(obj.clone(), true));
            menu_item(ui, acts, "Delete View", Act::DropTables(obj.cid, obj.db.clone(), vec![obj.name.clone()], true));
            menu_item(ui, acts, "Export Wizard…", Act::Export(obj.clone()));
        }
        ObjMode::Functions => menu_item(ui, acts, "Delete", Act::DropRoutine(obj.clone(), "FUNCTION".into())),
        ObjMode::Queries => {
            menu_item(ui, acts, "Open Query", Act::OpenSavedQuery(obj.clone()));
            menu_item(ui, acts, "Delete Query", Act::DeleteQuery(obj.clone()));
        }
        ObjMode::Backups => {
            menu_item(ui, acts, "Restore Backup", Act::RestoreBackup(obj.clone()));
            menu_item(ui, acts, "Delete Backup", Act::DeleteBackup(obj.clone()));
        }
        ObjMode::Users => {}
    }
    if matches!(mode, ObjMode::Tables | ObjMode::Views | ObjMode::Functions) && ui.button("Copy Name").clicked() {
        ui.ctx().copy_text(obj.name.clone());
        ui.close();
    }
}

fn table_menu(ui: &mut Ui, obj: &ObjRef, acts: &mut Vec<Act>) {
    let (cid, db, name) = (obj.cid, obj.db.clone(), obj.name.clone());
    menu_item(ui, acts, "Open Table", Act::OpenTable(obj.clone(), false));
    menu_item(ui, acts, "Design Table", Act::DesignTable(cid, db.clone(), Some(name.clone())));
    menu_item(ui, acts, "New Table", Act::DesignTable(cid, db.clone(), None));
    menu_item(ui, acts, "Delete Table", Act::DropTables(cid, db.clone(), vec![name.clone()], false));
    menu_item(ui, acts, "Truncate Table", Act::Truncate(obj.clone()));
    menu_item(ui, acts, "Rename", Act::Rename(obj.clone()));
    ui.separator();
    menu_item(ui, acts, "Import Wizard…", Act::Import(cid, db.clone(), Some(name.clone())));
    menu_item(ui, acts, "Export Wizard…", Act::Export(obj.clone()));
    ui.separator();
    menu_item(ui, acts, "New Query on This Table", Act::NewQuery(cid, db.clone(), format!("SELECT *\nFROM {}\nLIMIT 100;\n", name)));
    menu_item(ui, acts, "Refresh", Act::RefreshDb(cid, db));
}
