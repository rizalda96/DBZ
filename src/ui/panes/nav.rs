//! The connection tree: connections › databases › folders › objects.

use eframe::egui::{self, RichText, Ui};

use super::object_menus::{db_menu, object_menu, open_act};
use crate::app::{Act, ConnState, ConnStatus, DbState, FolderRef, ObjMode, ObjRef, Sel, Workspace};
use crate::domain::DbKind;
use crate::ui::icons::Icon;
use crate::ui::theme::{kind_color, pal};
use crate::ui::widgets::{context_menu, menu_item, tree_row, TreeItem};

const FOLDERS: [ObjMode; 5] = [ObjMode::Tables, ObjMode::Views, ObjMode::Functions, ObjMode::Queries, ObjMode::Backups];
/// Under every schema. Queries and Backups are saved per database, so each schema
/// shows the same files.
const SCHEMA_FOLDERS: [ObjMode; 6] = [ObjMode::Tables, ObjMode::Views, ObjMode::MatViews, ObjMode::Functions, ObjMode::Queries, ObjMode::Backups];

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
    let (cid, d) = (cx.cid, cx.d);
    let dsel = cx.selected_in(None) && cx.container_selected();
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
    if d.schemas.is_empty() {
        for mode in FOLDERS {
            folder_branch(ui, cx, Folder { schema: None, mode, depth: 2 }, acts);
        }
        return;
    }
    for schema in &d.schemas {
        schema_branch(ui, cx, schema, acts);
    }
}

/// The database whose folders are being drawn.
struct DbCtx<'a> {
    ws: &'a Workspace,
    cid: u64,
    d: &'a DbState,
    filter: &'a str,
}

impl DbCtx<'_> {
    /// True when this database (and `schema`) is selected without an object.
    fn selected_in(&self, schema: Option<&str>) -> bool {
        let sel = &self.ws.sel;
        sel.cid == Some(self.cid) && sel.db.as_deref() == Some(self.d.name.as_str()) && sel.obj.is_none() && sel.schema.as_deref() == schema
    }

    /// The database or schema row is highlighted rather than its Tables folder.
    fn container_selected(&self) -> bool {
        self.ws.active != 0 || self.ws.mode == ObjMode::Tables
    }

    fn folder_ref(&self, schema: Option<&str>, mode: ObjMode) -> FolderRef {
        FolderRef { cid: self.cid, db: self.d.name.clone(), schema: schema.map(str::to_string), mode }
    }
}

/// A folder row: under the database itself (`schema` None) or under one schema.
struct Folder<'a> {
    schema: Option<&'a str>,
    mode: ObjMode,
    depth: usize,
}

fn schema_branch(ui: &mut Ui, cx: &DbCtx, schema: &str, acts: &mut Vec<Act>) {
    let filtering = !cx.filter.is_empty();
    if filtering && SCHEMA_FOLDERS.iter().all(|&m| matching(cx, Some(schema), m).is_empty()) {
        return;
    }
    let open = cx.d.open_schemas.contains(schema) || filtering;
    let selected = cx.selected_in(Some(schema)) && cx.container_selected();
    let (resp, toggled) = tree_row(ui, TreeItem { depth: 2, expander: Some(open), icon: Icon::Schema, label: schema, selected });
    if toggled {
        acts.push(Act::ToggleSchema(cx.cid, cx.d.name.clone(), schema.to_string()));
    } else if resp.clicked() {
        acts.push(Act::SelectFolder(cx.folder_ref(Some(schema), ObjMode::Tables)));
    }
    if !open {
        return;
    }
    for mode in SCHEMA_FOLDERS {
        folder_branch(ui, cx, Folder { schema: Some(schema), mode, depth: 3 }, acts);
    }
}

/// Objects of `mode` in `schema` (all schemas when None) that pass the tree filter.
fn matching(cx: &DbCtx, schema: Option<&str>, mode: ObjMode) -> Vec<Leaf> {
    let d = cx.d;
    let in_schema = |s: &str| schema.is_none_or(|want| want == s);
    let items: Vec<Leaf> = match mode {
        ObjMode::Tables | ObjMode::Views | ObjMode::MatViews => d.tables.iter().filter(|t| mode.holds(t) && in_schema(&t.schema)).map(|t| (t.name.clone(), t.is_view, mode.icon())).collect(),
        ObjMode::Functions => d.routines.iter().filter(|r| in_schema(&r.schema)).map(|r| (r.name.clone(), false, Icon::Function)).collect(),
        ObjMode::Queries | ObjMode::Backups => d.files(mode).iter().map(|f| (f.name.clone(), false, mode.icon())).collect(),
        ObjMode::Users => vec![],
    };
    items.into_iter().filter(|i| cx.filter.is_empty() || i.0.to_lowercase().contains(cx.filter)).collect()
}

fn folder_branch(ui: &mut Ui, cx: &DbCtx, f: Folder, acts: &mut Vec<Act>) {
    let items = matching(cx, f.schema, f.mode);
    if !cx.filter.is_empty() && items.is_empty() {
        return;
    }
    let open = cx.d.open_folders.contains(&(f.schema.map(str::to_string), f.mode)) || !cx.filter.is_empty();
    let selected = cx.selected_in(f.schema) && cx.ws.showing(f.mode) && !cx.container_selected();
    let (resp, toggled) = tree_row(ui, TreeItem { depth: f.depth, expander: Some(open), icon: f.mode.icon(), label: f.mode.label(), selected });
    if toggled {
        acts.push(Act::ToggleFolder(cx.folder_ref(f.schema, f.mode)));
    } else if resp.clicked() {
        acts.push(Act::SelectFolder(cx.folder_ref(f.schema, f.mode)));
    }
    if !open {
        return;
    }
    for item in items {
        leaf(ui, cx, &f, item, acts);
    }
}

/// "sales.orders" shown as "orders" inside schema "sales"; other names stay as they are.
fn unqualified<'a>(name: &'a str, schema: &str) -> &'a str {
    name.strip_prefix(schema).and_then(|n| n.strip_prefix('.')).unwrap_or(name)
}

fn leaf(ui: &mut Ui, cx: &DbCtx, f: &Folder, (name, is_view, icon): Leaf, acts: &mut Vec<Act>) {
    let ws = cx.ws;
    let mode = f.mode;
    let obj = ObjRef::new(cx.cid, &cx.d.name, &name);
    let selected = ws.sel.cid == Some(obj.cid) && ws.sel.db.as_deref() == Some(obj.db.as_str()) && ws.sel.obj.as_deref() == Some(obj.name.as_str()) && ws.mode == mode;
    let label = f.schema.map_or(name.as_str(), |s| unqualified(&name, s));
    let (resp, _) = tree_row(ui, TreeItem { depth: f.depth + 1, expander: None, icon, label, selected });
    if resp.clicked() || resp.secondary_clicked() {
        acts.push(Act::SetMode(mode));
        acts.push(Act::Select(Sel::in_schema(obj.cid, &obj.db, f.schema.map(str::to_string), Some(obj.name.clone()))));
    }
    // Functions have nothing to open; their fallback would drop the schema selection.
    if resp.double_clicked() && mode != ObjMode::Functions {
        acts.push(open_act(mode, obj.clone(), is_view));
    }
    context_menu(&resp, |ui| object_menu(ui, mode, &obj, acts));
}

#[cfg(test)]
mod tests {
    use super::unqualified;

    #[test]
    fn strips_only_its_own_schema() {
        assert_eq!(unqualified("sales.orders", "sales"), "orders");
        assert_eq!(unqualified("orders", "public"), "orders");
        assert_eq!(unqualified("salesman", "sales"), "salesman");
        assert_eq!(unqualified("hr.staff", "sales"), "hr.staff");
    }
}
