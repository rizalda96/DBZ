//! Right-click menus for databases and objects, and what double-click opens.

use eframe::egui::{self, Ui};

use crate::app::{Act, ObjMode, ObjRef, Sel};
use crate::domain::RelationKind;
use crate::transfer::ArchivePart;
use crate::ui::widgets::menu_item;

/// What double-clicking an object in `mode` does.
pub(super) fn open_act(mode: ObjMode, obj: ObjRef, is_view: bool) -> Act {
    match mode {
        ObjMode::Tables | ObjMode::Views | ObjMode::MatViews => Act::OpenTable(obj, is_view),
        ObjMode::Queries => Act::OpenSavedQuery(obj),
        ObjMode::Backups => Act::RestoreBackup(obj),
        ObjMode::Functions => Act::DesignRoutine(obj),
        _ => Act::Select(Sel { obj: Some(obj.name), ..Sel::db(obj.cid, obj.db) }),
    }
}

/// The database a menu is for.
pub(super) struct DbTarget<'a> {
    pub cid: u64,
    pub db: &'a str,
    pub expanded: bool,
    /// The server has its own dump format (PostgreSQL `.dump`).
    pub archives: bool,
}

pub(super) fn db_menu(ui: &mut Ui, target: DbTarget, acts: &mut Vec<Act>) {
    let DbTarget { cid, db, expanded, archives } = target;
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
    if archives {
        archive_items(ui, cid, &db, acts);
    }
    menu_item(ui, acts, "Import Wizard…", Act::Import(cid, db.clone(), None));
    menu_item(ui, acts, "New Backup", Act::NewBackup(cid, db.clone()));
    ui.separator();
    refresh_item(ui, acts, Act::RefreshDb(cid, db));
}

/// "Refresh" with its F5 shortcut, at the end of every tree and object menu.
pub(super) fn refresh_item(ui: &mut Ui, acts: &mut Vec<Act>, act: Act) {
    if ui.add(egui::Button::new("Refresh").shortcut_text("F5")).clicked() {
        acts.push(act);
        ui.close();
    }
}

/// Dump and restore in the server's own format (database menu and Tools menu).
pub(super) fn archive_items(ui: &mut Ui, cid: u64, db: &str, acts: &mut Vec<Act>) {
    menu_item(ui, acts, "Dump Archive File (Structure + Data)…", Act::DumpArchive(cid, db.to_string(), ArchivePart::StructureAndData));
    menu_item(ui, acts, "Dump Archive File (Structure Only)…", Act::DumpArchive(cid, db.to_string(), ArchivePart::StructureOnly));
    menu_item(ui, acts, "Restore Archive File…", Act::RestoreArchive(cid, db.to_string()));
}

pub(super) fn object_menu(ui: &mut Ui, mode: ObjMode, obj: &ObjRef, acts: &mut Vec<Act>) {
    match mode {
        ObjMode::Tables => table_menu(ui, obj, acts),
        ObjMode::Views => {
            menu_item(ui, acts, "Open View", Act::OpenTable(obj.clone(), true));
            menu_item(ui, acts, "Design View", Act::DesignView(obj.clone()));
            menu_item(ui, acts, "Delete View", Act::DropTables(obj.cid, obj.db.clone(), vec![obj.name.clone()], RelationKind::View));
            menu_item(ui, acts, "Export Wizard…", Act::Export(obj.clone()));
        }
        ObjMode::MatViews => {
            menu_item(ui, acts, "Open Materialized View", Act::OpenTable(obj.clone(), true));
            menu_item(ui, acts, "New Materialized View", Act::NewMatView(obj.cid, obj.db.clone()));
            menu_item(ui, acts, "Delete Materialized View", Act::DropTables(obj.cid, obj.db.clone(), vec![obj.name.clone()], RelationKind::MatView));
            menu_item(ui, acts, "Export Wizard…", Act::Export(obj.clone()));
        }
        ObjMode::Functions => routine_menu(ui, obj, acts),
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
    if matches!(mode, ObjMode::Tables | ObjMode::Views | ObjMode::MatViews | ObjMode::Functions) && ui.button("Copy Name").clicked() {
        ui.ctx().copy_text(obj.name.clone());
        ui.close();
    }
    if mode != ObjMode::Users {
        ui.separator();
        refresh_item(ui, acts, Act::RefreshDb(obj.cid, obj.db.clone()));
    }
}

/// Functions and procedures. Delete names FUNCTION here; the app uses the real kind.
fn routine_menu(ui: &mut Ui, obj: &ObjRef, acts: &mut Vec<Act>) {
    menu_item(ui, acts, "Design Function", Act::DesignRoutine(obj.clone()));
    menu_item(ui, acts, "New Function", Act::NewRoutine(obj.cid, obj.db.clone()));
    menu_item(ui, acts, "Delete Function", Act::DropRoutine(obj.clone(), "FUNCTION".into()));
    menu_item(ui, acts, "Execute Function", Act::ExecRoutine(obj.clone()));
    ui.separator();
}

fn table_menu(ui: &mut Ui, obj: &ObjRef, acts: &mut Vec<Act>) {
    let (cid, db, name) = (obj.cid, obj.db.clone(), obj.name.clone());
    menu_item(ui, acts, "Open Table", Act::OpenTable(obj.clone(), false));
    menu_item(ui, acts, "Design Table", Act::DesignTable(cid, db.clone(), Some(name.clone())));
    menu_item(ui, acts, "New Table", Act::DesignTable(cid, db.clone(), None));
    menu_item(ui, acts, "Delete Table", Act::DropTables(cid, db.clone(), vec![name.clone()], RelationKind::Table));
    menu_item(ui, acts, "Truncate Table", Act::Truncate(obj.clone()));
    menu_item(ui, acts, "Rename", Act::Rename(obj.clone()));
    ui.separator();
    menu_item(ui, acts, "Import Wizard…", Act::Import(cid, db, Some(name)));
    menu_item(ui, acts, "Export Wizard…", Act::Export(obj.clone()));
    ui.separator();
    menu_item(ui, acts, "New Query on This Table", Act::QueryTable(obj.clone()));
}
