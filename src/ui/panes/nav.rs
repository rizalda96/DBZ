//! The connection tree: connections › databases › folders › objects.

use eframe::egui::{self, RichText, Ui};

use super::object_menus::{db_menu, object_menu, open_act};
use crate::app::{Act, ConnState, ConnStatus, DbState, ObjMode, ObjRef, Sel, Workspace};
use crate::domain::DbKind;
use crate::ui::icons::Icon;
use crate::ui::theme::{kind_color, pal};
use crate::ui::widgets::{context_menu, menu_item, tree_row, TreeItem};

const FOLDERS: [ObjMode; 5] = [ObjMode::Tables, ObjMode::Views, ObjMode::Functions, ObjMode::Queries, ObjMode::Backups];

/// One object leaf: name, whether it is a view, icon.
type Leaf = (String, bool, Icon);

pub fn nav(ui: &mut Ui, ws: &Workspace, filter_text: &mut String, acts: &mut Vec<Act>) {
    egui::Frame::new().inner_margin(egui::Margin::symmetric(6, 5)).show(ui, |ui| {
        ui.add(egui::TextEdit::singleline(filter_text).hint_text("Search objects").desired_width(f32::INFINITY));
    });
    let filter = filter_text.to_lowercase();
    egui::ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
        ui.spacing_mut().item_spacing.y = 0.0;
        if ws.conns.is_empty() {
            no_connections(ui, acts);
        }
        for c in &ws.conns {
            conn_branch(ui, ws, c, &filter, acts);
        }
    });
}

fn no_connections(ui: &mut Ui, acts: &mut Vec<Act>) {
    ui.add_space(20.0);
    ui.vertical_centered(|ui| {
        ui.label(RichText::new("No connections yet").color(pal().muted));
        if ui.button("New Connection…").clicked() {
            acts.push(Act::NewConn(DbKind::MySql));
        }
    });
}

fn conn_branch(ui: &mut Ui, ws: &Workspace, c: &ConnState, filter: &str, acts: &mut Vec<Act>) {
    let cid = c.cfg.id;
    let open = c.is_open();
    let sel = ws.sel.cid == Some(cid) && ws.sel.db.is_none();
    let label = if c.status == ConnStatus::Opening { format!("{}  (connecting…)", c.cfg.name) } else { c.cfg.name.clone() };
    let icon = Icon::Conn { color: kind_color(c.cfg.kind), open };
    let (resp, toggled) = tree_row(ui, TreeItem { depth: 0, expander: Some(c.expanded && open), icon, label: &label, selected: sel });
    if toggled {
        acts.push(Act::ToggleConn(cid));
    } else if resp.clicked() {
        acts.push(Act::Select(Sel::conn(cid)));
    }
    let resp = resp.on_hover_text(conn_tooltip(c));
    context_menu(&resp, |ui| conn_menu(ui, cid, open, acts));
    if !(c.expanded && open) {
        return;
    }
    if c.status == ConnStatus::Opening && c.dbs.is_empty() {
        indented_spinner(ui, 36.0, None);
    }
    for d in &c.dbs {
        db_branch(ui, &DbCtx { ws, cid, d, filter }, acts);
    }
}

fn conn_tooltip(c: &ConnState) -> String {
    if c.cfg.kind.is_builtin() {
        return "Built-in sample database".to_string();
    }
    format!("{} — {}:{} — user {}", c.cfg.kind.label(), c.cfg.host, c.cfg.port, c.cfg.user)
}

fn conn_menu(ui: &mut Ui, cid: u64, open: bool, acts: &mut Vec<Act>) {
    if open {
        menu_item(ui, acts, "Close Connection", Act::CloseConn(cid));
    } else {
        menu_item(ui, acts, "Open Connection", Act::OpenConn(cid));
    }
    menu_item(ui, acts, "Edit Connection…", Act::EditConn(cid));
    menu_item(ui, acts, "Duplicate Connection", Act::DupConn(cid));
    menu_item(ui, acts, "Delete Connection", Act::DeleteConn(cid));
    ui.separator();
    if open {
        menu_item(ui, acts, "New Database…", Act::NewDb(cid));
    }
    menu_item(ui, acts, "New Query", Act::NewQuery(cid, String::new(), String::new()));
    if open && ui.button("Users").clicked() {
        acts.push(Act::Select(Sel::conn(cid)));
        acts.push(Act::SetMode(ObjMode::Users));
        ui.close();
    }
    if open {
        ui.separator();
        menu_item(ui, acts, "Refresh", Act::RefreshConn(cid));
    }
}

fn indented_spinner(ui: &mut Ui, indent: f32, text: Option<&str>) {
    ui.horizontal(|ui| {
        ui.add_space(indent);
        ui.spinner();
        if let Some(t) = text {
            ui.label(RichText::new(t).color(pal().muted));
        }
    });
}

fn db_branch(ui: &mut Ui, cx: &DbCtx, acts: &mut Vec<Act>) {
    let (ws, cid, d) = (cx.ws, cx.cid, cx.d);
    let in_db = ws.sel.cid == Some(cid) && ws.sel.db.as_deref() == Some(d.name.as_str()) && ws.sel.obj.is_none();
    let dsel = in_db && (ws.active != 0 || ws.mode == ObjMode::Tables);
    let (resp, toggled) = tree_row(ui, TreeItem { depth: 1, expander: Some(d.expanded), icon: Icon::Database { open: d.expanded }, label: &d.name, selected: dsel });
    if toggled {
        acts.push(Act::ToggleDb(cid, d.name.clone()));
    } else if resp.clicked() {
        acts.push(Act::Select(Sel::db(cid, &d.name)));
        if d.loaded {
            acts.push(Act::SetMode(ObjMode::Tables));
        }
    }
    context_menu(&resp, |ui| db_menu(ui, cid, &d.name, d.expanded, acts));
    if !d.expanded {
        return;
    }
    if d.loading && !d.loaded {
        indented_spinner(ui, 52.0, Some("Loading…"));
        return;
    }
    for mode in FOLDERS {
        let fsel = in_db && ws.showing(mode) && !dsel;
        folder_branch(ui, cx, mode, fsel, acts);
    }
}

/// The database whose folders are being drawn.
struct DbCtx<'a> {
    ws: &'a Workspace,
    cid: u64,
    d: &'a DbState,
    filter: &'a str,
}

fn leaves(d: &DbState, mode: ObjMode) -> Vec<Leaf> {
    match mode {
        ObjMode::Tables => d.tables.iter().filter(|t| !t.is_view).map(|t| (t.name.clone(), false, Icon::Table)).collect(),
        ObjMode::Views => d.tables.iter().filter(|t| t.is_view).map(|t| (t.name.clone(), true, Icon::View)).collect(),
        ObjMode::Functions => d.routines.iter().map(|r| (r.name.clone(), false, Icon::Function)).collect(),
        ObjMode::Queries | ObjMode::Backups => d.files(mode).iter().map(|f| (f.name.clone(), false, mode.icon())).collect(),
        ObjMode::Users => vec![],
    }
}

fn folder_branch(ui: &mut Ui, cx: &DbCtx, mode: ObjMode, selected: bool, acts: &mut Vec<Act>) {
    let (d, filter) = (cx.d, cx.filter);
    let items: Vec<Leaf> = leaves(d, mode).into_iter().filter(|i| filter.is_empty() || i.0.to_lowercase().contains(filter)).collect();
    if !filter.is_empty() && items.is_empty() {
        return;
    }
    let open = d.open_folders.contains(&mode) || !filter.is_empty();
    let (resp, toggled) = tree_row(ui, TreeItem { depth: 2, expander: Some(open), icon: Icon::Folder, label: mode.label(), selected });
    if toggled {
        acts.push(Act::ToggleFolder(cx.cid, d.name.clone(), mode));
    } else if resp.clicked() {
        acts.push(Act::SelectFolder(cx.cid, d.name.clone(), mode));
    }
    if !open {
        return;
    }
    for item in items {
        leaf(ui, cx, mode, item, acts);
    }
}

fn leaf(ui: &mut Ui, cx: &DbCtx, mode: ObjMode, (name, is_view, icon): Leaf, acts: &mut Vec<Act>) {
    let ws = cx.ws;
    let obj = ObjRef::new(cx.cid, &cx.d.name, &name);
    let selected = ws.sel.cid == Some(obj.cid) && ws.sel.db.as_deref() == Some(obj.db.as_str()) && ws.sel.obj.as_deref() == Some(obj.name.as_str()) && ws.mode == mode;
    let (resp, _) = tree_row(ui, TreeItem { depth: 3, expander: None, icon, label: &obj.name, selected });
    if resp.clicked() || resp.secondary_clicked() {
        acts.push(Act::SetMode(mode));
        acts.push(Act::Select(Sel { cid: Some(obj.cid), db: Some(obj.db.clone()), obj: Some(obj.name.clone()) }));
    }
    if resp.double_clicked() {
        acts.push(open_act(mode, obj.clone(), is_view));
    }
    context_menu(&resp, |ui| object_menu(ui, mode, &obj, acts));
}
