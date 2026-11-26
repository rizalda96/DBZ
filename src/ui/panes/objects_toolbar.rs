//! Toolbar of the Objects tab; its buttons depend on the object kind shown.

use eframe::egui::{self, Ui};

use super::search::{self, SearchBox};
use crate::app::{Act, ObjMode, ObjRef, Workspace};
use crate::domain::RelationKind;
use crate::sql::dialect_for;
use crate::ui::icons::Icon;
use crate::ui::widgets::{tool_btn, tool_sep, toolbar_frame};

/// The selection the buttons act on.
struct Target {
    has_db: bool,
    has_obj: bool,
    obj: ObjRef,
}

pub(super) fn show(ui: &mut Ui, ws: &Workspace, filter: &mut String, acts: &mut Vec<Act>) {
    let obj = ObjRef { cid: ws.sel.cid.unwrap_or(0), db: ws.sel.db.clone().unwrap_or_default(), name: ws.sel.obj.clone().unwrap_or_default() };
    let has_db = ws.sel.cid.is_some() && ws.sel.db.is_some();
    let t = Target { has_db, has_obj: has_db && ws.sel.obj.is_some(), obj };
    toolbar_frame().show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 2.0;
            let mut b = |ui: &mut Ui, icon: Icon, text: &str, enabled: bool, act: Act| {
                if tool_btn(ui, Some(icon), text, enabled).clicked() {
                    acts.push(act);
                }
            };
            match ws.mode {
                ObjMode::Tables => table_buttons(ui, &t, ws.sel.obj.clone(), &mut b),
                ObjMode::Views | ObjMode::MatViews | ObjMode::Functions | ObjMode::Users => code_buttons(ui, ws, &t, &mut b),
                ObjMode::Queries | ObjMode::Backups => file_buttons(ui, ws, &t, &mut b),
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let hint = search::hint(ui.ctx(), "Search");
                ui.add(egui::TextEdit::singleline(filter).id(SearchBox::Objects.id()).hint_text(hint).desired_width(170.0));
            });
        });
    });
}

type Button<'a> = dyn FnMut(&mut Ui, Icon, &str, bool, Act) + 'a;

fn table_buttons(ui: &mut Ui, t: &Target, sel_obj: Option<String>, b: &mut Button) {
    let o = &t.obj;
    b(ui, Icon::Table, "Open Table", t.has_obj, Act::OpenTable(o.clone(), false));
    b(ui, Icon::Design, "Design Table", t.has_obj, Act::DesignTable(o.cid, o.db.clone(), Some(o.name.clone())));
    b(ui, Icon::Add, "New Table", t.has_db, Act::DesignTable(o.cid, o.db.clone(), None));
    b(ui, Icon::Delete, "Delete Table", t.has_obj, Act::DropTables(o.cid, o.db.clone(), vec![o.name.clone()], RelationKind::Table));
    tool_sep(ui);
    b(ui, Icon::Import, "Import Wizard", t.has_db, Act::Import(o.cid, o.db.clone(), sel_obj));
    b(ui, Icon::Export, "Export Wizard", t.has_obj, Act::Export(o.clone()));
}

/// Views, functions and users: buttons that start from a SQL template.
fn code_buttons(ui: &mut Ui, ws: &Workspace, t: &Target, b: &mut Button) {
    let o = &t.obj;
    let tpl = dialect_for(ws.kind_of(o.cid)).templates();
    let has_conn = ws.sel.cid.is_some();
    match ws.mode {
        ObjMode::Views => {
            b(ui, Icon::View, "Open View", t.has_obj, Act::OpenTable(o.clone(), true));
            b(ui, Icon::Design, "Design View", t.has_obj, Act::DesignView(o.clone()));
            b(ui, Icon::Add, "New View", t.has_db, Act::NewQuery(o.cid, o.db.clone(), tpl.new_view.into()));
            b(ui, Icon::Delete, "Delete View", t.has_obj, Act::DropTables(o.cid, o.db.clone(), vec![o.name.clone()], RelationKind::View));
        }
        ObjMode::MatViews => {
            b(ui, Icon::MatView, "Open Materialized View", t.has_obj, Act::OpenTable(o.clone(), true));
            b(ui, Icon::Add, "New Materialized View", t.has_db, Act::NewMatView(o.cid, o.db.clone()));
            b(ui, Icon::Delete, "Delete Materialized View", t.has_obj, Act::DropTables(o.cid, o.db.clone(), vec![o.name.clone()], RelationKind::MatView));
        }
        ObjMode::Functions => {
            b(ui, Icon::Add, "New Function", t.has_db, Act::NewQuery(o.cid, o.db.clone(), tpl.new_function.into()));
            b(ui, Icon::Delete, "Delete Function", t.has_obj, Act::DropRoutine(o.clone(), routine_kind(ws, o)));
        }
        _ => {
            b(ui, Icon::Refresh, "Refresh", has_conn, Act::LoadUsers(o.cid));
            b(ui, Icon::Add, "New User", has_conn, Act::NewQuery(o.cid, o.db.clone(), tpl.new_user.into()));
        }
    }
}

/// FUNCTION or PROCEDURE, as the server reported it.
fn routine_kind(ws: &Workspace, o: &ObjRef) -> String {
    let routine = ws.conn(o.cid).and_then(|c| c.db(&o.db)).and_then(|d| d.routines.iter().find(|r| r.name == o.name));
    routine.map(|r| r.kind.clone()).unwrap_or_else(|| "FUNCTION".into())
}

fn file_buttons(ui: &mut Ui, ws: &Workspace, t: &Target, b: &mut Button) {
    let o = &t.obj;
    if ws.mode == ObjMode::Queries {
        b(ui, Icon::NewQuery, "New Query", ws.sel.cid.is_some(), Act::NewQuery(o.cid, o.db.clone(), String::new()));
        b(ui, Icon::Query, "Open Query", t.has_obj, Act::OpenSavedQuery(o.clone()));
        b(ui, Icon::Delete, "Delete Query", t.has_obj, Act::DeleteQuery(o.clone()));
    } else {
        b(ui, Icon::Backup, "New Backup", t.has_db, Act::NewBackup(o.cid, o.db.clone()));
        b(ui, Icon::Refresh, "Restore Backup", t.has_obj, Act::RestoreBackup(o.clone()));
        b(ui, Icon::Delete, "Delete Backup", t.has_obj, Act::DeleteBackup(o.clone()));
    }
}
