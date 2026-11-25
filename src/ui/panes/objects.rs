//! The Objects tab: databases of a connection, or the objects of a database.

use eframe::egui::{self, RichText, Ui};

use super::object_menus::{db_menu, object_menu, open_act};
use super::objects_toolbar;
use crate::app::{Act, ConnState, DbState, ObjMode, ObjRef, Sel, Workspace};
use crate::domain::DbKind;
use crate::shared::fmt_bytes;
use crate::ui::icons::{self, Icon};
use crate::ui::theme::pal;
use crate::ui::widgets::{result_grid, ObjRow, ObjectTable, RowEv};

const WELCOME_KINDS: [DbKind; 3] = [DbKind::MySql, DbKind::MariaDb, DbKind::PostgreSql];
const TABLE_HEADERS: [&str; 6] = ["Name", "Rows", "Data Length", "Engine", "Modified Date", "Comment"];
const VIEW_HEADERS: [&str; 3] = ["Name", "Owner / Engine", "Comment"];

pub fn objects(ui: &mut Ui, ws: &Workspace, filter_text: &mut String, acts: &mut Vec<Act>) {
    objects_toolbar::show(ui, ws, filter_text, acts);
    breadcrumb(ui, ws);
    let filter = filter_text.to_lowercase();
    let Some(cid) = ws.sel.cid else {
        welcome(ui, acts);
        return;
    };
    let Some(c) = ws.conn(cid) else { return };
    if ws.mode == ObjMode::Users {
        users(ui, c, acts);
        return;
    }
    let Some(dbn) = ws.sel.db.clone() else {
        databases(ui, c, &filter, acts);
        return;
    };
    let Some(d) = c.db(&dbn) else { return };
    if !d.loaded {
        not_loaded(ui, cid, d, acts);
        return;
    }
    let list = ObjectList { ws, cid, d, filter: &filter };
    if let Some((name, is_view, open)) = list.show(ui, acts) {
        let obj = ObjRef::new(cid, &dbn, &name);
        acts.push(if open { open_act(ws.mode, obj, is_view) } else { Act::Select(Sel::in_schema(cid, dbn, ws.sel.schema.clone(), Some(name))) });
    }
}

fn breadcrumb(ui: &mut Ui, ws: &Workspace) {
    let Some(c) = ws.sel.cid.and_then(|c| ws.conn(c)) else { return };
    let crumb = match &ws.sel.db {
        Some(d) if ws.mode != ObjMode::Users => {
            let schema = ws.sel.schema.as_ref().map(|s| format!("   ›   {}", s)).unwrap_or_default();
            format!("{}   ›   {}{}   ›   {}", c.cfg.name, d, schema, ws.mode.label())
        }
        _ => format!("{}   ›   {}", c.cfg.name, if ws.mode == ObjMode::Users { "Users" } else { "Databases" }),
    };
    egui::Frame::new().inner_margin(egui::Margin::symmetric(10, 4)).show(ui, |ui| {
        ui.label(RichText::new(crumb).color(pal().muted).size(12.0));
    });
}

fn welcome(ui: &mut Ui, acts: &mut Vec<Act>) {
    ui.add_space(60.0);
    ui.vertical_centered(|ui| {
        icons::logo(ui, 96.0);
        ui.add_space(10.0);
        ui.label(RichText::new("Welcome to DBZ").size(20.0).strong());
        ui.label(RichText::new("Select a connection on the left, or create a new one.").color(pal().muted));
        ui.add_space(12.0);
        for k in WELCOME_KINDS {
            if ui.button(format!("New {} Connection…", k.label())).clicked() {
                acts.push(Act::NewConn(k));
            }
        }
    });
}

fn users(ui: &mut Ui, c: &ConnState, acts: &mut Vec<Act>) {
    match &c.users {
        None => {
            ui.horizontal(|ui| {
                ui.add_space(10.0);
                if c.is_open() {
                    ui.spinner();
                    ui.label("Loading users…");
                } else if ui.button("Open Connection").clicked() {
                    acts.push(Act::OpenConn(c.cfg.id));
                }
            });
        }
        Some(Err(e)) => {
            ui.colored_label(pal().err, format!("  {}", e));
        }
        Some(Ok(rs)) => result_grid(ui, ("users", c.cfg.id), &rs.columns, &rs.rows, &mut None),
    }
}

fn databases(ui: &mut Ui, c: &ConnState, filter: &str, acts: &mut Vec<Act>) {
    let cid = c.cfg.id;
    if !c.is_open() {
        ui.add_space(20.0);
        ui.vertical_centered(|ui| {
            ui.label(RichText::new(format!("\"{}\" is not open.", c.cfg.name)).color(pal().muted));
            if ui.button("Open Connection").clicked() {
                acts.push(Act::OpenConn(cid));
            }
        });
        return;
    }
    let rows: Vec<&DbState> = c.dbs.iter().filter(|d| filter.is_empty() || d.name.to_lowercase().contains(filter)).collect();
    let mut open = None;
    let row = |i: usize| {
        let d = rows[i];
        let tables = if d.loaded { d.tables.len().to_string() } else { String::new() };
        let status = if d.expanded { "Open".into() } else { String::new() };
        ObjRow { cells: vec![d.name.clone(), tables, status], icon: Icon::Database { open: false }, selected: false }
    };
    ObjectTable { headers: &["Name", "Tables", "Status"], rows: rows.len() }.show(ui, "dblist", row, |i, e| match e {
        RowEv::Double => open = Some(i),
        RowEv::Click => {}
        RowEv::Menu(ui) => db_menu(ui, cid, &rows[i].name, rows[i].expanded, acts),
    });
    if let Some(i) = open {
        acts.push(Act::OpenDb(cid, rows[i].name.clone()));
    }
}

fn not_loaded(ui: &mut Ui, cid: u64, d: &DbState, acts: &mut Vec<Act>) {
    ui.horizontal(|ui| {
        ui.add_space(10.0);
        if d.loading {
            ui.spinner();
            ui.label("Loading objects…");
        } else if let Some(e) = &d.error {
            ui.colored_label(pal().err, e);
        } else if ui.button("Open Database").clicked() {
            acts.push(Act::OpenDb(cid, d.name.clone()));
        }
    });
}

fn empty_note(ui: &mut Ui, text: &str) {
    ui.add_space(6.0);
    ui.label(RichText::new(format!("   {}", text)).color(pal().muted));
}

/// The objects of one database in the current mode.
struct ObjectList<'a> {
    ws: &'a Workspace,
    cid: u64,
    d: &'a DbState,
    filter: &'a str,
}

/// One listed object: name, is-view, cells.
type Listed = (String, bool, Vec<String>);

impl ObjectList<'_> {
    /// Draws the list; returns (name, is_view, double-clicked) of a clicked row.
    fn show(&self, ui: &mut Ui, acts: &mut Vec<Act>) -> Option<(String, bool, bool)> {
        let mode = self.ws.mode;
        let (headers, items, empty): (&[&str], Vec<Listed>, &str) = match mode {
            ObjMode::Tables => (&TABLE_HEADERS, self.tables(mode), "No tables yet. Press New Table to create one."),
            ObjMode::Views => (&VIEW_HEADERS, self.tables(mode), "No views in this database."),
            ObjMode::MatViews => (&VIEW_HEADERS, self.tables(mode), "No materialized views in this schema."),
            ObjMode::Functions => (&["Name", "Type", "Returns"], self.routines(), "No functions or procedures in this database."),
            ObjMode::Queries => (&["Name", "Size", "Modified Date"], self.files(mode), "No saved queries yet. Open a New Query and press Save."),
            ObjMode::Backups => (&["Name", "Size", "Modified Date"], self.files(mode), "No backups yet. Press New Backup to make one."),
            ObjMode::Users => return None,
        };
        if items.is_empty() && self.filter.is_empty() {
            empty_note(ui, empty);
        }
        let selected = self.ws.sel.obj.as_deref();
        let icon = if mode == ObjMode::Tables { Icon::Table } else if mode == ObjMode::Views { Icon::View } else { mode.icon() };
        let row = |i: usize| ObjRow { cells: items[i].2.clone(), icon, selected: selected == Some(items[i].0.as_str()) };
        let mut hit = None;
        ObjectTable { headers, rows: items.len() }.show(ui, ("olist", mode as u8), row, |i, ev| match ev {
            RowEv::Click => hit = Some((items[i].0.clone(), items[i].1, false)),
            RowEv::Double if mode != ObjMode::Functions => hit = Some((items[i].0.clone(), items[i].1, true)),
            RowEv::Double => {}
            RowEv::Menu(ui) => object_menu(ui, mode, &ObjRef::new(self.cid, &self.d.name, &items[i].0), acts),
        });
        hit
    }

    fn matches(&self, name: &str) -> bool {
        self.filter.is_empty() || name.to_lowercase().contains(self.filter)
    }

    /// True unless another schema than `schema` is selected.
    fn in_schema(&self, schema: &str) -> bool {
        self.ws.sel.schema.as_deref().is_none_or(|s| s == schema)
    }

    fn tables(&self, mode: ObjMode) -> Vec<Listed> {
        let tables = self.d.tables.iter().filter(|t| mode.holds(t) && self.in_schema(&t.schema) && self.matches(&t.name));
        if mode != ObjMode::Tables {
            return tables.map(|t| (t.name.clone(), true, vec![t.name.clone(), t.engine.clone(), t.comment.clone()])).collect();
        }
        tables
            .map(|t| {
                let modified = if t.modified.is_empty() { t.created.clone() } else { t.modified.clone() };
                let cells = vec![t.name.clone(), t.rows.map(|r| r.to_string()).unwrap_or_default(), t.data_length.map(fmt_bytes).unwrap_or_default(), t.engine.clone(), modified, t.comment.clone()];
                (t.name.clone(), false, cells)
            })
            .collect()
    }

    fn routines(&self) -> Vec<Listed> {
        self.d.routines.iter().filter(|r| self.in_schema(&r.schema) && self.matches(&r.name)).map(|r| (r.name.clone(), false, vec![r.name.clone(), r.kind.clone(), r.returns.clone()])).collect()
    }

    fn files(&self, mode: ObjMode) -> Vec<Listed> {
        self.d.files(mode).iter().filter(|f| self.matches(&f.name)).map(|f| (f.name.clone(), false, vec![f.name.clone(), fmt_bytes(f.size), f.modified.clone()])).collect()
    }
}
