//! Window pieces: menu, ribbon, connection tree, object list, info pane and tab bar.

use std::path::PathBuf;

use eframe::egui::{self, pos2, vec2, Color32, FontId, Rect, RichText, Sense, Stroke, Ui};
use egui_extras::{Column, TableBuilder};

use crate::app::*;
use crate::backend::Req;
use crate::icons::{self, Icon};
use crate::model::*;
use crate::views::{self, *};

pub const ERR: Color32 = Color32::from_rgb(0xb0, 0x20, 0x20);

impl Dbz {
    pub fn menu_bar(&mut self, ui: &mut Ui, acts: &mut Vec<Act>) {
        egui::MenuBar::new().ui(ui, |ui| {
            ui.menu_button("File", |ui| {
                ui.menu_button("New Connection", |ui| {
                    for k in DbKind::ALL {
                        if ui.button(k.label()).clicked() {
                            acts.push(Act::NewConn(k));
                            ui.close();
                        }
                    }
                });
                if ui.add(egui::Button::new("New Query").shortcut_text("Ctrl+Q")).clicked() {
                    match self.sel.cid.or(self.conns.first().map(|c| c.cfg.id)) {
                        Some(cid) => acts.push(Act::NewQuery(cid, self.sel.db.clone().unwrap_or_default(), String::new())),
                        None => acts.push(Act::Alert("New Query".into(), "Create a connection first.".into())),
                    }
                    ui.close();
                }
                if ui.button("Open SQL File…").clicked() {
                    ui.close();
                    if let Some(p) = rfd::FileDialog::new().add_filter("SQL file", &["sql", "txt"]).pick_file() {
                        match std::fs::read_to_string(&p) {
                            Ok(sql) => {
                                if let Some(cid) = self.sel.cid.or(self.conns.first().map(|c| c.cfg.id)) {
                                    acts.push(Act::NewQuery(cid, self.sel.db.clone().unwrap_or_default(), sql));
                                }
                            }
                            Err(e) => acts.push(Act::Alert("Open SQL File".into(), e.to_string())),
                        }
                    }
                }
                ui.separator();
                let has_db = self.sel.cid.is_some() && self.sel.db.is_some();
                if ui.add_enabled(has_db, egui::Button::new("Execute SQL File…")).clicked() {
                    acts.push(Act::RunFile(self.sel.cid.unwrap(), self.sel.db.clone().unwrap()));
                    ui.close();
                }
                ui.separator();
                if ui.add_enabled(self.active > 0, egui::Button::new("Close Tab").shortcut_text("Ctrl+W")).clicked() {
                    acts.push(Act::CloseTab(self.active));
                    ui.close();
                }
                if ui.button("Exit").clicked() {
                    ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                }
            });
            ui.menu_button("View", |ui| {
                if ui.checkbox(&mut self.show_info, "Information Pane").clicked() {
                    ui.close();
                }
                ui.separator();
                for m in [ObjMode::Tables, ObjMode::Views, ObjMode::Functions, ObjMode::Users, ObjMode::Queries, ObjMode::Backups] {
                    if ui.selectable_label(self.mode == m && self.active == 0, m.label()).clicked() {
                        acts.push(Act::SetMode(m));
                        ui.close();
                    }
                }
                ui.separator();
                if ui.add(egui::Button::new("Refresh").shortcut_text("F5")).clicked() {
                    self.refresh_current(acts);
                    ui.close();
                }
            });
            ui.menu_button("Tools", |ui| {
                let (cid, db, obj) = (self.sel.cid, self.sel.db.clone(), self.sel.obj.clone());
                let has_db = cid.is_some() && db.is_some();
                let table = if self.mode == ObjMode::Tables { obj.clone() } else { None };
                if ui.add_enabled(has_db, egui::Button::new("Import Wizard…")).clicked() {
                    acts.push(Act::Import(cid.unwrap(), db.clone().unwrap(), table.clone()));
                    ui.close();
                }
                if ui.add_enabled(has_db && table.is_some(), egui::Button::new("Export Wizard…")).clicked() {
                    acts.push(Act::Export(cid.unwrap(), db.clone().unwrap(), table.clone().unwrap()));
                    ui.close();
                }
                ui.separator();
                if ui.add_enabled(has_db, egui::Button::new("Dump SQL File (Structure + Data)…")).clicked() {
                    ui.close();
                    acts.push(Act::Dump(cid.unwrap(), db.clone().unwrap(), true));
                }
                if ui.add_enabled(has_db, egui::Button::new("Dump SQL File (Structure Only)…")).clicked() {
                    ui.close();
                    acts.push(Act::Dump(cid.unwrap(), db.clone().unwrap(), false));
                }
                if ui.add_enabled(has_db, egui::Button::new("Execute SQL File…")).clicked() {
                    acts.push(Act::RunFile(cid.unwrap(), db.clone().unwrap()));
                    ui.close();
                }
                ui.separator();
                if ui.add_enabled(has_db, egui::Button::new("New Backup")).clicked() {
                    acts.push(Act::NewBackup(cid.unwrap(), db.clone().unwrap()));
                    acts.push(Act::SetMode(ObjMode::Backups));
                    ui.close();
                }
            });
            ui.menu_button("Window", |ui| {
                if ui.selectable_label(self.active == 0, "Objects").clicked() {
                    acts.push(Act::Activate(0));
                    ui.close();
                }
                for (i, t) in self.tabs.iter().enumerate() {
                    if ui.selectable_label(self.active == i + 1, t.title(&self.conns)).clicked() {
                        acts.push(Act::Activate(i + 1));
                        ui.close();
                    }
                }
                ui.separator();
                if ui.button("Close All Tabs").clicked() {
                    for i in (1..=self.tabs.len()).rev() {
                        acts.push(Act::CloseTab(i));
                    }
                    ui.close();
                }
            });
            ui.menu_button("Help", |ui| {
                if ui.button("Keyboard Shortcuts").clicked() {
                    acts.push(Act::Alert(
                        "Keyboard Shortcuts".into(),
                        "Ctrl+Q — New query\nCtrl+R or F9 — Run query (or just the selected text)\nCtrl+S — Save (query, data changes, table design)\nCtrl+W — Close tab\nF5 — Refresh\nEnter or F2 — Edit the selected cell\nArrow keys — Move between cells\nCtrl+C — Copy the selected cell".into(),
                    ));
                    ui.close();
                }
                if ui.button("About DBZ").clicked() {
                    acts.push(Act::ShowAbout);
                    ui.close();
                }
            });
        });
    }

    pub fn ribbon(&mut self, ui: &mut Ui, acts: &mut Vec<Act>) {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 2.0;
            let r = ribbon_btn(ui, Icon::Connection, "Connection", false, true);
            egui::Popup::menu(&r).show(|ui| {
                ui.set_min_width(200.0);
                for k in DbKind::ALL {
                    if ui.button(k.label()).clicked() {
                        acts.push(Act::NewConn(k));
                        ui.close();
                    }
                }
            });
            let qcid = self.sel.cid.or(self.conns.first().map(|c| c.cfg.id));
            if ribbon_btn(ui, Icon::NewQuery, "New Query", false, qcid.is_some()).clicked() {
                acts.push(Act::NewQuery(qcid.unwrap(), self.sel.db.clone().unwrap_or_default(), String::new()));
            }
            ribbon_sep(ui);
            for (m, label) in [(ObjMode::Tables, "Table"), (ObjMode::Views, "View"), (ObjMode::Functions, "Function"), (ObjMode::Users, "User")] {
                if ribbon_btn(ui, m.icon(), label, self.mode == m && self.active == 0, true).clicked() {
                    acts.push(Act::SetMode(m));
                }
            }
            ribbon_sep(ui);
            for (m, label) in [(ObjMode::Queries, "Query"), (ObjMode::Backups, "Backup")] {
                if ribbon_btn(ui, m.icon(), label, self.mode == m && self.active == 0, true).clicked() {
                    acts.push(Act::SetMode(m));
                }
            }
            ribbon_sep(ui);
            let has_db = self.sel.cid.is_some() && self.sel.db.is_some();
            if ribbon_btn(ui, Icon::Import, "Import", false, has_db).clicked() {
                acts.push(Act::Import(self.sel.cid.unwrap(), self.sel.db.clone().unwrap(), if self.mode == ObjMode::Tables { self.sel.obj.clone() } else { None }));
            }
            let can_export = has_db && self.sel.obj.is_some() && self.mode == ObjMode::Tables;
            if ribbon_btn(ui, Icon::Export, "Export", false, can_export).clicked() {
                acts.push(Act::Export(self.sel.cid.unwrap(), self.sel.db.clone().unwrap(), self.sel.obj.clone().unwrap()));
            }
        });
    }

    // ------------------------------------------------------------ navigation tree

    pub fn nav(&mut self, ui: &mut Ui, acts: &mut Vec<Act>) {
        egui::Frame::new().inner_margin(egui::Margin::symmetric(6, 5)).show(ui, |ui| {
            ui.add(egui::TextEdit::singleline(&mut self.tree_filter).hint_text("Search objects").desired_width(f32::INFINITY));
        });
        let filter = self.tree_filter.to_lowercase();
        egui::ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 0.0;
            if self.conns.is_empty() {
                ui.add_space(20.0);
                ui.vertical_centered(|ui| {
                    ui.label(RichText::new("No connections yet").color(MUTED));
                    if ui.button("New Connection…").clicked() {
                        acts.push(Act::NewConn(DbKind::MySql));
                    }
                });
            }
            for c in &self.conns {
                let cid = c.cfg.id;
                let open = c.tx.is_some();
                let sel = self.sel.cid == Some(cid) && self.sel.db.is_none();
                let label = if c.status == ConnStatus::Opening { format!("{}  (connecting…)", c.cfg.name) } else { c.cfg.name.clone() };
                let (resp, toggled) = tree_row(ui, 0, Some(c.expanded && open), Icon::Conn { color: c.cfg.kind.color(), open }, &label, sel);
                if toggled {
                    acts.push(Act::ToggleConn(cid));
                } else if resp.clicked() {
                    acts.push(Act::Select(Sel { cid: Some(cid), db: None, obj: None }));
                }
                let tip = if c.cfg.kind == DbKind::Sample { "Built-in sample database".to_string() } else { format!("{} — {}:{} — user {}", c.cfg.kind.label(), c.cfg.host, c.cfg.port, c.cfg.user) };
                let resp = resp.on_hover_text(tip);
                resp.context_menu(|ui| {
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
                        acts.push(Act::Select(Sel { cid: Some(cid), db: None, obj: None }));
                        acts.push(Act::SetMode(ObjMode::Users));
                        ui.close();
                    }
                    if open {
                        ui.separator();
                        menu_item(ui, acts, "Refresh", Act::RefreshConn(cid));
                    }
                });
                if !(c.expanded && open) {
                    continue;
                }
                if c.status == ConnStatus::Opening && c.dbs.is_empty() {
                    ui.horizontal(|ui| {
                        ui.add_space(36.0);
                        ui.spinner();
                    });
                }
                for d in &c.dbs {
                    let db = d.name.clone();
                    let dsel = self.sel.cid == Some(cid) && self.sel.db.as_deref() == Some(db.as_str()) && self.sel.obj.is_none() && (self.active != 0 || self.mode == ObjMode::Tables);
                    let (resp, toggled) = tree_row(ui, 1, Some(d.expanded), Icon::Database { open: d.expanded }, &d.name, dsel);
                    if toggled {
                        acts.push(Act::ToggleDb(cid, db.clone()));
                    } else if resp.clicked() {
                        acts.push(Act::Select(Sel { cid: Some(cid), db: Some(db.clone()), obj: None }));
                        if d.loaded {
                            acts.push(Act::SetMode(ObjMode::Tables));
                        }
                    }
                    resp.context_menu(|ui| db_menu(ui, cid, &db, d.expanded, acts));
                    if !d.expanded {
                        continue;
                    }
                    if d.loading && !d.loaded {
                        ui.horizontal(|ui| {
                            ui.add_space(52.0);
                            ui.spinner();
                            ui.label(RichText::new("Loading…").color(MUTED));
                        });
                        continue;
                    }
                    let folders: [(ObjMode, Vec<(String, bool, Icon)>); 5] = [
                        (ObjMode::Tables, d.tables.iter().filter(|t| !t.is_view).map(|t| (t.name.clone(), false, Icon::Table)).collect()),
                        (ObjMode::Views, d.tables.iter().filter(|t| t.is_view).map(|t| (t.name.clone(), true, Icon::View)).collect()),
                        (ObjMode::Functions, d.routines.iter().map(|r| (r.name.clone(), false, Icon::Function)).collect()),
                        (ObjMode::Queries, d.queries.iter().map(|q| (q.0.clone(), false, Icon::Query)).collect()),
                        (ObjMode::Backups, d.backups.iter().map(|q| (q.0.clone(), false, Icon::Backup)).collect()),
                    ];
                    for (mode, items) in folders {
                        let items: Vec<_> = items.into_iter().filter(|i| filter.is_empty() || i.0.to_lowercase().contains(&filter)).collect();
                        if !filter.is_empty() && items.is_empty() {
                            continue;
                        }
                        let fopen = d.open_folders.contains(&mode) || !filter.is_empty();
                        let fsel = self.sel.cid == Some(cid) && self.sel.db.as_deref() == Some(db.as_str()) && self.sel.obj.is_none() && self.mode == mode && self.active == 0 && !dsel;
                        let (resp, toggled) = tree_row(ui, 2, Some(fopen), Icon::Folder, mode.label(), fsel);
                        if toggled {
                            acts.push(Act::ToggleFolder(cid, db.clone(), mode));
                        } else if resp.clicked() {
                            acts.push(Act::SelectFolder(cid, db.clone(), mode));
                        }
                        if !fopen {
                            continue;
                        }
                        for (name, is_view, icon) in items {
                            let osel = self.sel.cid == Some(cid) && self.sel.db.as_deref() == Some(db.as_str()) && self.sel.obj.as_deref() == Some(name.as_str()) && self.mode == mode;
                            let (resp, _) = tree_row(ui, 3, None, icon, &name, osel);
                            if resp.clicked() || resp.secondary_clicked() {
                                acts.push(Act::SetMode(mode));
                                acts.push(Act::Select(Sel { cid: Some(cid), db: Some(db.clone()), obj: Some(name.clone()) }));
                            }
                            if resp.double_clicked() {
                                acts.push(open_act(mode, cid, &db, &name, is_view));
                            }
                            resp.context_menu(|ui| object_menu(ui, mode, cid, &db, &name, acts));
                        }
                    }
                }
            }
        });
    }

    // ------------------------------------------------------------ objects tab

    pub fn objects(&mut self, ui: &mut Ui, acts: &mut Vec<Act>) {
        let (cid, db, obj) = (self.sel.cid, self.sel.db.clone(), self.sel.obj.clone());
        let mode = self.mode;
        let pg = cid.map(|c| self.conn(c).map(|c| c.cfg.kind.is_pg()).unwrap_or(false)).unwrap_or(false);
        toolbar_frame().show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 2.0;
                let has_db = cid.is_some() && db.is_some();
                let has_obj = has_db && obj.is_some();
                let c = cid.unwrap_or(0);
                let d = db.clone().unwrap_or_default();
                let o = obj.clone().unwrap_or_default();
                let mut b = |ui: &mut Ui, icon: Icon, text: &str, enabled: bool, act: Act| {
                    if tool_btn(ui, Some(icon), text, enabled).clicked() {
                        acts.push(act);
                    }
                };
                match mode {
                    ObjMode::Tables => {
                        b(ui, Icon::Table, "Open Table", has_obj, Act::OpenTable(c, d.clone(), o.clone(), false));
                        b(ui, Icon::Design, "Design Table", has_obj, Act::DesignTable(c, d.clone(), Some(o.clone())));
                        b(ui, Icon::Add, "New Table", has_db, Act::DesignTable(c, d.clone(), None));
                        b(ui, Icon::Delete, "Delete Table", has_obj, Act::DropTables(c, d.clone(), vec![o.clone()], false));
                        tsep(ui);
                        b(ui, Icon::Import, "Import Wizard", has_db, Act::Import(c, d.clone(), obj.clone()));
                        b(ui, Icon::Export, "Export Wizard", has_obj, Act::Export(c, d.clone(), o.clone()));
                    }
                    ObjMode::Views => {
                        let q = if pg { "\"" } else { "`" };
                        b(ui, Icon::View, "Open View", has_obj, Act::OpenTable(c, d.clone(), o.clone(), true));
                        b(ui, Icon::Add, "New View", has_db, Act::NewQuery(c, d.clone(), format!("CREATE VIEW {q}new_view{q} AS\nSELECT *\nFROM {q}table_name{q};\n")));
                        b(ui, Icon::Delete, "Delete View", has_obj, Act::DropTables(c, d.clone(), vec![o.clone()], true));
                    }
                    ObjMode::Functions => {
                        let tpl = if pg {
                            "CREATE OR REPLACE FUNCTION add_numbers(a integer, b integer)\nRETURNS integer AS $$\nBEGIN\n  RETURN a + b;\nEND;\n$$ LANGUAGE plpgsql;\n"
                        } else {
                            "CREATE FUNCTION add_numbers(a INT, b INT)\nRETURNS INT DETERMINISTIC\nRETURN a + b;\n"
                        };
                        let kind = self.conn(c).and_then(|cs| cs.db(&d)).and_then(|ds| ds.routines.iter().find(|r| r.name == o)).map(|r| r.kind.clone()).unwrap_or_else(|| "FUNCTION".into());
                        b(ui, Icon::Add, "New Function", has_db, Act::NewQuery(c, d.clone(), tpl.into()));
                        b(ui, Icon::Delete, "Delete Function", has_obj, Act::DropRoutine(c, d.clone(), o.clone(), kind));
                    }
                    ObjMode::Queries => {
                        b(ui, Icon::NewQuery, "New Query", cid.is_some(), Act::NewQuery(c, d.clone(), String::new()));
                        b(ui, Icon::Query, "Open Query", has_obj, Act::OpenSavedQuery(c, d.clone(), o.clone()));
                        b(ui, Icon::Delete, "Delete Query", has_obj, Act::DeleteQuery(c, d.clone(), o.clone()));
                    }
                    ObjMode::Backups => {
                        b(ui, Icon::Backup, "New Backup", has_db, Act::NewBackup(c, d.clone()));
                        b(ui, Icon::Refresh, "Restore Backup", has_obj, Act::RestoreBackup(c, d.clone(), o.clone()));
                        b(ui, Icon::Delete, "Delete Backup", has_obj, Act::DeleteBackup(c, d.clone(), o.clone()));
                    }
                    ObjMode::Users => {
                        let tpl = if pg { "CREATE ROLE new_user WITH LOGIN;\n" } else { "CREATE USER 'new_user'@'%';\nGRANT SELECT ON *.* TO 'new_user'@'%';\n" };
                        b(ui, Icon::Refresh, "Refresh", cid.is_some(), Act::LoadUsers(c));
                        b(ui, Icon::Add, "New User", cid.is_some(), Act::NewQuery(c, d.clone(), tpl.into()));
                    }
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.add(egui::TextEdit::singleline(&mut self.obj_filter).hint_text("Search").desired_width(170.0));
                });
            });
        });
        let crumb = match (cid.and_then(|c| self.conn(c)), &db) {
            (Some(c), Some(d)) if mode != ObjMode::Users => format!("{}   ›   {}   ›   {}", c.cfg.name, d, mode.label()),
            (Some(c), _) => format!("{}   ›   {}", c.cfg.name, if mode == ObjMode::Users { "Users" } else { "Databases" }),
            _ => String::new(),
        };
        if !crumb.is_empty() {
            egui::Frame::new().inner_margin(egui::Margin::symmetric(10, 4)).show(ui, |ui| {
                ui.label(RichText::new(crumb).color(MUTED).size(12.0));
            });
        }
        let filter = self.obj_filter.to_lowercase();
        let Some(cid) = cid else {
            welcome(ui, acts);
            return;
        };
        let Some(c) = self.conn(cid) else { return };
        if mode == ObjMode::Users {
            match &c.users {
                None => {
                    ui.horizontal(|ui| {
                        ui.add_space(10.0);
                        if c.tx.is_some() {
                            ui.spinner();
                            ui.label("Loading users…");
                        } else if ui.button("Open Connection").clicked() {
                            acts.push(Act::OpenConn(cid));
                        }
                    });
                }
                Some(Err(e)) => {
                    ui.colored_label(ERR, format!("  {}", e));
                }
                Some(Ok(rs)) => {
                    let mut sel = None;
                    views::result_grid(ui, ("users", cid), &rs.columns, &rs.rows, &mut sel);
                }
            }
            return;
        }
        let Some(dbn) = db else {
            if c.tx.is_none() {
                ui.add_space(20.0);
                ui.vertical_centered(|ui| {
                    ui.label(RichText::new(format!("\"{}\" is not open.", c.cfg.name)).color(MUTED));
                    if ui.button("Open Connection").clicked() {
                        acts.push(Act::OpenConn(cid));
                    }
                });
                return;
            }
            let rows: Vec<&DbState> = c.dbs.iter().filter(|d| filter.is_empty() || d.name.to_lowercase().contains(&filter)).collect();
            let mut ev: Option<usize> = None;
            object_table(ui, "dblist", &["Name", "Tables", "Status"], rows.len(), |i| vec![rows[i].name.clone(), if rows[i].loaded { rows[i].tables.len().to_string() } else { String::new() }, if rows[i].expanded { "Open".into() } else { String::new() }], |_| Icon::Database { open: false }, |_| false, |i, e| match e {
                RowEv::Double => ev = Some(i),
                RowEv::Click => {}
                RowEv::Menu(ui) => db_menu(ui, cid, &rows[i].name, rows[i].expanded, acts),
            });
            if let Some(i) = ev {
                acts.push(Act::OpenDb(cid, rows[i].name.clone()));
            }
            return;
        };
        let Some(d) = c.db(&dbn) else { return };
        if !d.loaded {
            ui.horizontal(|ui| {
                ui.add_space(10.0);
                if d.loading {
                    ui.spinner();
                    ui.label("Loading objects…");
                } else if let Some(e) = &d.error {
                    ui.colored_label(ERR, e);
                } else if ui.button("Open Database").clicked() {
                    acts.push(Act::OpenDb(cid, dbn.clone()));
                }
            });
            return;
        }
        let sel_name = obj.clone();
        let mut clicked: Option<String> = None;
        let mut doubled: Option<(String, bool)> = None;
        let m = |s: &str| filter.is_empty() || s.to_lowercase().contains(&filter);
        match mode {
            ObjMode::Tables | ObjMode::Views => {
                let rows: Vec<&TableInfo> = d.tables.iter().filter(|t| t.is_view == (mode == ObjMode::Views) && m(&t.name)).collect();
                if rows.is_empty() && filter.is_empty() {
                    empty_note(ui, if mode == ObjMode::Tables { "No tables yet. Press New Table to create one." } else { "No views in this database." });
                }
                let headers: &[&str] = if mode == ObjMode::Tables { &["Name", "Rows", "Data Length", "Engine", "Modified Date", "Comment"] } else { &["Name", "Owner / Engine", "Comment"] };
                object_table(
                    ui,
                    ("tlist", mode as u8),
                    headers,
                    rows.len(),
                    |i| {
                        let t = rows[i];
                        if mode == ObjMode::Tables {
                            vec![t.name.clone(), t.rows.map(|r| r.to_string()).unwrap_or_default(), t.data_length.map(fmt_bytes).unwrap_or_default(), t.engine.clone(), if t.modified.is_empty() { t.created.clone() } else { t.modified.clone() }, t.comment.clone()]
                        } else {
                            vec![t.name.clone(), t.engine.clone(), t.comment.clone()]
                        }
                    },
                    |_| if mode == ObjMode::Tables { Icon::Table } else { Icon::View },
                    |i| sel_name.as_deref() == Some(rows[i].name.as_str()),
                    |i, ev| match ev {
                        RowEv::Click => clicked = Some(rows[i].name.clone()),
                        RowEv::Double => doubled = Some((rows[i].name.clone(), rows[i].is_view)),
                        RowEv::Menu(ui) => object_menu(ui, mode, cid, &dbn, &rows[i].name, acts),
                    },
                );
            }
            ObjMode::Functions => {
                let rows: Vec<&RoutineInfo> = d.routines.iter().filter(|r| m(&r.name)).collect();
                if rows.is_empty() && filter.is_empty() {
                    empty_note(ui, "No functions or procedures in this database.");
                }
                object_table(ui, "flist", &["Name", "Type", "Returns"], rows.len(), |i| vec![rows[i].name.clone(), rows[i].kind.clone(), rows[i].returns.clone()], |_| Icon::Function, |i| sel_name.as_deref() == Some(rows[i].name.as_str()), |i, ev| match ev {
                    RowEv::Click => clicked = Some(rows[i].name.clone()),
                    RowEv::Double => {}
                    RowEv::Menu(ui) => object_menu(ui, mode, cid, &dbn, &rows[i].name, acts),
                });
            }
            ObjMode::Queries | ObjMode::Backups => {
                let list = if mode == ObjMode::Queries { &d.queries } else { &d.backups };
                let rows: Vec<&(String, u64, String)> = list.iter().filter(|q| m(&q.0)).collect();
                if rows.is_empty() && filter.is_empty() {
                    empty_note(ui, if mode == ObjMode::Queries { "No saved queries yet. Open a New Query and press Save." } else { "No backups yet. Press New Backup to make one." });
                }
                object_table(ui, ("qlist", mode as u8), &["Name", "Size", "Modified Date"], rows.len(), |i| vec![rows[i].0.clone(), fmt_bytes(rows[i].1), rows[i].2.clone()], |_| mode.icon(), |i| sel_name.as_deref() == Some(rows[i].0.as_str()), |i, ev| match ev {
                    RowEv::Click => clicked = Some(rows[i].0.clone()),
                    RowEv::Double => doubled = Some((rows[i].0.clone(), false)),
                    RowEv::Menu(ui) => object_menu(ui, mode, cid, &dbn, &rows[i].0, acts),
                });
            }
            ObjMode::Users => {}
        }
        if let Some(n) = clicked {
            acts.push(Act::Select(Sel { cid: Some(cid), db: Some(dbn.clone()), obj: Some(n) }));
        }
        if let Some((n, v)) = doubled {
            acts.push(open_act(mode, cid, &dbn, &n, v));
        }
    }

    // ------------------------------------------------------------ info pane

    pub fn info(&mut self, ui: &mut Ui, acts: &mut Vec<Act>) {
        let (cid, db, obj) = (self.sel.cid, self.sel.db.clone(), self.sel.obj.clone());
        let mode = self.mode;
        let table_sel = obj.is_some() && matches!(mode, ObjMode::Tables | ObjMode::Views);
        egui::Frame::new().fill(HEADER_BG).inner_margin(egui::Margin::symmetric(8, 4)).show(ui, |ui| {
            ui.horizontal(|ui| {
                if ui.selectable_label(!self.info_ddl || !table_sel, "General").clicked() {
                    self.info_ddl = false;
                }
                if ui.add_enabled(table_sel, egui::Button::selectable(self.info_ddl && table_sel, "DDL")).clicked() {
                    self.info_ddl = true;
                }
            });
        });
        let Some(cid) = cid else {
            ui.add_space(30.0);
            ui.vertical_centered(|ui| ui.label(RichText::new("No object selected").color(MUTED)));
            return;
        };
        let mut want_ddl = None;
        let Some(c) = self.conn(cid) else { return };
        egui::ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
            egui::Frame::new().inner_margin(egui::Margin::symmetric(12, 10)).show(ui, |ui| {
                let header = |ui: &mut Ui, icon: Icon, title: &str, sub: &str| {
                    ui.horizontal(|ui| {
                        let (r, _) = ui.allocate_exact_size(vec2(40.0, 40.0), Sense::hover());
                        icons::paint(ui.painter(), r, icon);
                        ui.vertical(|ui| {
                            ui.add(egui::Label::new(RichText::new(title).strong().size(15.0)).truncate());
                            ui.label(RichText::new(sub).color(MUTED));
                        });
                    });
                    ui.add_space(12.0);
                };
                let prop = |ui: &mut Ui, k: &str, v: &str| {
                    ui.label(RichText::new(k).color(MUTED).size(12.0));
                    ui.add(egui::Label::new(if v.is_empty() { "—".to_string() } else { v.to_string() }).wrap());
                    ui.add_space(7.0);
                };
                if mode == ObjMode::Users || db.is_none() {
                    header(ui, Icon::Conn { color: c.cfg.kind.color(), open: c.tx.is_some() }, &c.cfg.name, c.cfg.kind.label());
                    let st = match &c.status {
                        ConnStatus::Closed => "Closed".to_string(),
                        ConnStatus::Opening => "Connecting…".to_string(),
                        ConnStatus::Open => "Open".to_string(),
                        ConnStatus::Failed(e) => format!("Failed: {}", e),
                    };
                    prop(ui, "Status", &st);
                    if c.cfg.kind != DbKind::Sample {
                        prop(ui, "Host", &c.cfg.host);
                        prop(ui, "Port", &c.cfg.port.to_string());
                        prop(ui, "User Name", &c.cfg.user);
                    } else {
                        prop(ui, "Location", "Built into this app (changes last until you close it)");
                    }
                    prop(ui, "Server Version", &c.version);
                    prop(ui, "Databases", &if c.tx.is_some() { c.dbs.len().to_string() } else { String::new() });
                    return;
                }
                let dbn = db.clone().unwrap();
                let d = c.db(&dbn);
                let Some(on) = obj.clone() else {
                    header(ui, Icon::Database { open: true }, &dbn, "Database");
                    if let Some(d) = d {
                        prop(ui, "Tables", &d.tables.iter().filter(|t| !t.is_view).count().to_string());
                        prop(ui, "Views", &d.tables.iter().filter(|t| t.is_view).count().to_string());
                        prop(ui, "Functions", &d.routines.len().to_string());
                        let size: u64 = d.tables.iter().filter_map(|t| t.data_length).sum();
                        prop(ui, "Data Length", &fmt_bytes(size));
                        prop(ui, "Saved Queries", &d.queries.len().to_string());
                        prop(ui, "Backups", &d.backups.len().to_string());
                    }
                    return;
                };
                match mode {
                    ObjMode::Tables | ObjMode::Views => {
                        let t = d.and_then(|d| d.tables.iter().find(|t| t.name == on));
                        let is_view = t.map(|t| t.is_view).unwrap_or(false);
                        if self.info_ddl {
                            let key = (cid, dbn.clone(), on.clone());
                            match self.ddl_cache.get(&key) {
                                None => {
                                    ui.spinner();
                                    want_ddl = Some((key, is_view));
                                }
                                Some(Ok(s)) if s.is_empty() => {
                                    ui.spinner();
                                }
                                Some(Ok(s)) => {
                                    let mut s = s.clone();
                                    ui.add(egui::TextEdit::multiline(&mut s).font(FontId::monospace(12.0)).desired_width(f32::INFINITY).frame(egui::Frame::NONE));
                                    if ui.button("Copy DDL").clicked() {
                                        ui.ctx().copy_text(s);
                                    }
                                }
                                Some(Err(e)) => {
                                    ui.colored_label(ERR, e);
                                }
                            }
                            return;
                        }
                        header(ui, if is_view { Icon::View } else { Icon::Table }, &on, if is_view { "View" } else { "Table" });
                        if let Some(t) = t {
                            if !is_view {
                                prop(ui, "Rows", &t.rows.map(|r| r.to_string()).unwrap_or_default());
                            }
                            prop(ui, if c.cfg.kind.is_pg() { "Owner" } else { "Engine" }, t.engine.trim_start_matches("owner: "));
                            prop(ui, "Data Length", &t.data_length.map(fmt_bytes).unwrap_or_default());
                            if !c.cfg.kind.is_pg() {
                                prop(ui, "Created Date", &t.created);
                                prop(ui, "Modified Date", &t.modified);
                                prop(ui, "Collation", &t.collation);
                            }
                            prop(ui, "Comment", &t.comment);
                        }
                    }
                    ObjMode::Functions => {
                        let r = d.and_then(|d| d.routines.iter().find(|r| r.name == on));
                        header(ui, Icon::Function, &on, &r.map(|r| r.kind.clone()).unwrap_or_default());
                        if let Some(r) = r {
                            prop(ui, "Returns", &r.returns);
                        }
                    }
                    ObjMode::Queries | ObjMode::Backups => {
                        let kind = if mode == ObjMode::Queries { "queries" } else { "backups" };
                        header(ui, mode.icon(), &on, if mode == ObjMode::Queries { "Saved Query" } else { "Backup" });
                        let list = d.map(|d| if mode == ObjMode::Queries { &d.queries } else { &d.backups });
                        if let Some(item) = list.and_then(|l| l.iter().find(|q| q.0 == on)) {
                            prop(ui, "Size", &fmt_bytes(item.1));
                            prop(ui, "Modified Date", &item.2);
                        }
                        let path = self.folder(kind, cid, &dbn).join(format!("{}.sql", on));
                        prop(ui, "File", &path.to_string_lossy());
                        if mode == ObjMode::Queries {
                            if let Ok(text) = std::fs::read_to_string(&path) {
                                let preview: String = text.chars().take(1200).collect();
                                ui.label(RichText::new(preview).monospace().size(11.5));
                            }
                        }
                    }
                    ObjMode::Users => {}
                }
            });
        });
        if let Some((key, is_view)) = want_ddl {
            acts.push(Act::Send(cid, Req::Ddl { db: key.1.clone(), table: key.2.clone(), is_view }));
            self.ddl_cache.insert(key, Ok(String::new()));
        }
    }

    pub fn status_bar(&self, ui: &mut Ui) {
        ui.horizontal_centered(|ui| {
            ui.add_space(4.0);
            if self.conns.iter().any(|c| c.status == ConnStatus::Opening) {
                ui.spinner();
            }
            ui.add(egui::Label::new(RichText::new(&self.status).size(12.0)).truncate());
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_space(6.0);
                if let Some(c) = self.sel.cid.and_then(|c| self.conn(c)) {
                    let mut parts = vec![c.cfg.name.clone()];
                    if let Some(db) = &self.sel.db {
                        parts.push(db.clone());
                        if let Some(d) = c.db(db) {
                            if d.loaded {
                                parts.push(format!("{} table(s)", d.tables.iter().filter(|t| !t.is_view).count()));
                            }
                        }
                    }
                    if let Some(o) = &self.sel.obj {
                        parts.push(o.clone());
                    }
                    ui.label(RichText::new(parts.join("    |    ")).size(12.0).color(MUTED));
                }
            });
        });
    }

    pub fn tab_bar(&mut self, ui: &mut Ui, acts: &mut Vec<Act>) {
        let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 31.0), Sense::hover());
        ui.painter().rect_filled(rect, 0.0, Color32::from_rgb(0xec, 0xee, 0xf1));
        ui.painter().line_segment([rect.left_bottom(), rect.right_bottom()], Stroke::new(1.0, BORDER));
        let mut x = rect.left() + 4.0;
        let n = self.tabs.len() + 1;
        let max_w = ((rect.width() - 8.0) / n as f32).clamp(90.0, 280.0);
        for i in 0..n {
            let (title, icon, id) = if i == 0 { ("Objects".to_string(), Icon::Folder, 0) } else { (self.tabs[i - 1].title(&self.conns), self.tabs[i - 1].icon(), self.tabs[i - 1].id()) };
            let active = self.active == i;
            let font = FontId::proportional(12.5);
            let galley = ui.painter().layout_no_wrap(title.clone(), font.clone(), TEXT);
            let w = (galley.size().x + if i == 0 { 40.0 } else { 58.0 }).min(max_w);
            if x + w > rect.right() {
                break;
            }
            let tr = Rect::from_min_size(pos2(x, rect.top() + 4.0), vec2(w, rect.height() - 4.0));
            let resp = ui.interact(tr, ui.scope_id().with(("tab", id)), Sense::click());
            let p = ui.painter().with_clip_rect(tr);
            let fill = if active { Color32::WHITE } else if resp.hovered() { Color32::from_rgb(0xf6, 0xf7, 0xf9) } else { Color32::from_rgb(0xe2, 0xe5, 0xe9) };
            p.rect_filled(tr, egui::CornerRadius { nw: 4, ne: 4, sw: 0, se: 0 }, fill);
            if active {
                p.rect_filled(Rect::from_min_size(tr.min, vec2(tr.width(), 2.5)), 0.0, ACCENT);
            }
            icons::paint(&p, Rect::from_center_size(pos2(tr.left() + 15.0, tr.center().y), vec2(15.0, 15.0)), icon);
            let text_clip = Rect::from_min_max(tr.min, pos2(tr.right() - if i == 0 { 4.0 } else { 26.0 }, tr.bottom()));
            ui.painter().with_clip_rect(text_clip).text(pos2(tr.left() + 28.0, tr.center().y), egui::Align2::LEFT_CENTER, &title, font, if active { TEXT } else { Color32::from_rgb(0x50, 0x56, 0x5e) });
            let mut closed = false;
            if i > 0 {
                let cr = Rect::from_center_size(pos2(tr.right() - 13.0, tr.center().y), vec2(16.0, 16.0));
                let hover_close = resp.hover_pos().map(|p| cr.contains(p)).unwrap_or(false);
                if hover_close {
                    p.rect_filled(cr, 3.0, Color32::from_rgb(0xd6, 0xdb, 0xe2));
                }
                if active || resp.hovered() {
                    p.text(cr.center(), egui::Align2::CENTER_CENTER, "×", FontId::proportional(15.0), MUTED);
                }
                if (resp.clicked() && hover_close) || resp.middle_clicked() {
                    acts.push(Act::CloseTab(i));
                    closed = true;
                }
            }
            if resp.clicked() && !closed {
                acts.push(Act::Activate(i));
            }
            if i > 0 {
                resp.on_hover_text(&title).context_menu(|ui| menu_item(ui, acts, "Close", Act::CloseTab(i)));
            }
            x += w + 2.0;
        }
    }
}

// ---------------------------------------------------------------- helpers

pub fn default_dir() -> PathBuf {
    std::env::var_os("USERPROFILE").map(|p| PathBuf::from(p).join("Documents")).unwrap_or_else(|| PathBuf::from("."))
}

pub fn fmt_time(secs: i64) -> String {
    let days = secs.div_euclid(86400);
    let rem = secs.rem_euclid(86400);
    let z = days + 719468;
    let era = z.div_euclid(146097);
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + if m <= 2 { 1 } else { 0 };
    format!("{:04}-{:02}-{:02} {:02}:{:02}", y, m, d, rem / 3600, rem % 3600 / 60)
}

fn tsep(ui: &mut Ui) {
    let (r, _) = ui.allocate_exact_size(vec2(9.0, 22.0), Sense::hover());
    ui.painter().line_segment([pos2(r.center().x, r.top() + 3.0), pos2(r.center().x, r.bottom() - 3.0)], Stroke::new(1.0, BORDER));
}

fn open_act(mode: ObjMode, cid: u64, db: &str, name: &str, is_view: bool) -> Act {
    match mode {
        ObjMode::Tables | ObjMode::Views => Act::OpenTable(cid, db.to_string(), name.to_string(), is_view),
        ObjMode::Queries => Act::OpenSavedQuery(cid, db.to_string(), name.to_string()),
        ObjMode::Backups => Act::RestoreBackup(cid, db.to_string(), name.to_string()),
        _ => Act::Select(Sel { cid: Some(cid), db: Some(db.to_string()), obj: Some(name.to_string()) }),
    }
}

pub fn menu_item(ui: &mut Ui, acts: &mut Vec<Act>, label: &str, act: Act) {
    if ui.button(label).clicked() {
        acts.push(act);
        ui.close();
    }
}

fn db_menu(ui: &mut Ui, cid: u64, db: &str, expanded: bool, acts: &mut Vec<Act>) {
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

fn object_menu(ui: &mut Ui, mode: ObjMode, cid: u64, db: &str, name: &str, acts: &mut Vec<Act>) {
    let db = db.to_string();
    let name = name.to_string();
    match mode {
        ObjMode::Tables => {
            menu_item(ui, acts, "Open Table", Act::OpenTable(cid, db.clone(), name.clone(), false));
            menu_item(ui, acts, "Design Table", Act::DesignTable(cid, db.clone(), Some(name.clone())));
            menu_item(ui, acts, "New Table", Act::DesignTable(cid, db.clone(), None));
            menu_item(ui, acts, "Delete Table", Act::DropTables(cid, db.clone(), vec![name.clone()], false));
            menu_item(ui, acts, "Truncate Table", Act::Truncate(cid, db.clone(), name.clone()));
            menu_item(ui, acts, "Rename", Act::Rename(cid, db.clone(), name.clone()));
            ui.separator();
            menu_item(ui, acts, "Import Wizard…", Act::Import(cid, db.clone(), Some(name.clone())));
            menu_item(ui, acts, "Export Wizard…", Act::Export(cid, db.clone(), name.clone()));
            ui.separator();
            menu_item(ui, acts, "New Query on This Table", Act::NewQuery(cid, db.clone(), format!("SELECT *\nFROM {}\nLIMIT 100;\n", name)));
            menu_item(ui, acts, "Refresh", Act::RefreshDb(cid, db.clone()));
        }
        ObjMode::Views => {
            menu_item(ui, acts, "Open View", Act::OpenTable(cid, db.clone(), name.clone(), true));
            menu_item(ui, acts, "Delete View", Act::DropTables(cid, db.clone(), vec![name.clone()], true));
            menu_item(ui, acts, "Export Wizard…", Act::Export(cid, db.clone(), name.clone()));
        }
        ObjMode::Functions => {
            menu_item(ui, acts, "Delete", Act::DropRoutine(cid, db.clone(), name.clone(), "FUNCTION".into()));
        }
        ObjMode::Queries => {
            menu_item(ui, acts, "Open Query", Act::OpenSavedQuery(cid, db.clone(), name.clone()));
            menu_item(ui, acts, "Delete Query", Act::DeleteQuery(cid, db.clone(), name.clone()));
        }
        ObjMode::Backups => {
            menu_item(ui, acts, "Restore Backup", Act::RestoreBackup(cid, db.clone(), name.clone()));
            menu_item(ui, acts, "Delete Backup", Act::DeleteBackup(cid, db.clone(), name.clone()));
        }
        ObjMode::Users => {}
    }
    if matches!(mode, ObjMode::Tables | ObjMode::Views | ObjMode::Functions) && ui.button("Copy Name").clicked() {
        ui.ctx().copy_text(name);
        ui.close();
    }
}

fn welcome(ui: &mut Ui, acts: &mut Vec<Act>) {
    ui.add_space(60.0);
    ui.vertical_centered(|ui| {
        icons::logo(ui, 96.0);
        ui.add_space(10.0);
        ui.label(RichText::new("Welcome to DBZ").size(20.0).strong());
        ui.label(RichText::new("Select a connection on the left, or create a new one.").color(MUTED));
        ui.add_space(12.0);
        for k in [DbKind::MySql, DbKind::MariaDb, DbKind::PostgreSql] {
            if ui.button(format!("New {} Connection…", k.label())).clicked() {
                acts.push(Act::NewConn(k));
            }
        }
    });
}

fn empty_note(ui: &mut Ui, text: &str) {
    ui.add_space(6.0);
    ui.label(RichText::new(format!("   {}", text)).color(MUTED));
}

pub enum RowEv<'a> {
    Click,
    Double,
    Menu(&'a mut Ui),
}

#[allow(clippy::too_many_arguments)]
fn object_table(
    ui: &mut Ui,
    id: impl std::hash::Hash + std::fmt::Debug,
    headers: &[&str],
    n: usize,
    cells: impl Fn(usize) -> Vec<String>,
    icon: impl Fn(usize) -> Icon,
    selected: impl Fn(usize) -> bool,
    mut on: impl FnMut(usize, RowEv),
) {
    ui.push_id(id, |ui| {
        ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
        let mut tb = TableBuilder::new(ui).striped(false).sense(Sense::click()).resizable(true).auto_shrink(false).cell_layout(egui::Layout::left_to_right(egui::Align::Center));
        for i in 0..headers.len() {
            tb = tb.column(if i == 0 {
                Column::initial(230.0).at_least(120.0).clip(true)
            } else if i + 1 == headers.len() {
                Column::remainder().at_least(80.0).clip(true)
            } else {
                Column::initial(120.0).at_least(50.0).clip(true)
            });
        }
        tb.header(26.0, |mut h| {
            for l in headers {
                h.col(|ui| {
                    let r = ui.max_rect();
                    ui.painter().line_segment([r.left_bottom(), r.right_bottom()], Stroke::new(1.0, BORDER));
                    ui.painter().line_segment([r.right_top() + vec2(0.0, 5.0), r.right_bottom() - vec2(0.0, 5.0)], Stroke::new(1.0, GRID_LINE));
                    ui.add_space(8.0);
                    ui.label(RichText::new(*l).color(Color32::from_rgb(0x55, 0x5b, 0x64)).size(12.5));
                });
            }
        })
        .body(|body| {
            body.rows(24.0, n, |mut row| {
                let i = row.index();
                let sel = selected(i);
                let vals = cells(i);
                let mut all: Option<egui::Response> = None;
                for (c, v) in vals.iter().enumerate() {
                    let (_, resp) = row.col(|ui| {
                        let r = ui.max_rect();
                        if sel {
                            ui.painter().rect_filled(r, 0.0, SEL_BG);
                        }
                        ui.add_space(8.0);
                        if c == 0 {
                            let (ir, _) = ui.allocate_exact_size(vec2(16.0, 16.0), Sense::hover());
                            icons::paint(ui.painter(), ir, icon(i));
                            ui.add_space(6.0);
                        }
                        let color = if c == 0 { TEXT } else { Color32::from_rgb(0x5a, 0x60, 0x68) };
                        ui.add(egui::Label::new(RichText::new(v).color(color)).truncate().selectable(false));
                    });
                    all = Some(match all {
                        None => resp,
                        Some(a) => a.union(resp),
                    });
                }
                if let Some(r) = all {
                    if r.double_clicked() {
                        on(i, RowEv::Double);
                    } else if r.clicked() || r.secondary_clicked() {
                        on(i, RowEv::Click);
                    }
                    r.context_menu(|ui| on(i, RowEv::Menu(ui)));
                }
            });
        });
    });
}

fn tree_row(ui: &mut Ui, depth: usize, expander: Option<bool>, icon: Icon, label: &str, selected: bool) -> (egui::Response, bool) {
    let w = ui.available_width();
    let (rect, resp) = ui.allocate_exact_size(vec2(w, 22.0), Sense::click());
    let p = ui.painter();
    if selected {
        p.rect_filled(rect, 0.0, SEL_BG);
    } else if resp.hovered() {
        p.rect_filled(rect, 0.0, Color32::from_rgb(0xee, 0xf3, 0xfa));
    }
    let x0 = rect.left() + 4.0 + depth as f32 * 16.0;
    let mut toggled = false;
    if let Some(open) = expander {
        let tri = Rect::from_center_size(pos2(x0 + 6.0, rect.center().y), vec2(12.0, 12.0));
        let c = tri.center();
        let col = Color32::from_rgb(0x80, 0x86, 0x8f);
        let pts = if open { vec![pos2(c.x - 4.0, c.y - 2.0), pos2(c.x + 4.0, c.y - 2.0), pos2(c.x, c.y + 3.0)] } else { vec![pos2(c.x - 2.0, c.y - 4.0), pos2(c.x + 3.0, c.y), pos2(c.x - 2.0, c.y + 4.0)] };
        p.add(egui::Shape::convex_polygon(pts, col, Stroke::NONE));
        if resp.clicked() {
            if let Some(pos) = resp.interact_pointer_pos() {
                if pos.x < tri.right() + 2.0 {
                    toggled = true;
                }
            }
        }
        if resp.double_clicked() {
            toggled = true;
        }
    }
    icons::paint(p, Rect::from_center_size(pos2(x0 + 22.0, rect.center().y), vec2(16.0, 16.0)), icon);
    p.with_clip_rect(rect).text(pos2(x0 + 34.0, rect.center().y), egui::Align2::LEFT_CENTER, label, FontId::proportional(13.0), TEXT);
    (resp, toggled)
}

fn ribbon_btn(ui: &mut Ui, icon: Icon, label: &str, selected: bool, enabled: bool) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(66.0, 60.0), if enabled { Sense::click() } else { Sense::hover() });
    let p = ui.painter();
    if selected {
        p.rect_filled(rect, 4.0, SEL_BG);
        p.rect_stroke(rect, 4.0, Stroke::new(1.0, Color32::from_rgb(0x9c, 0xc0, 0xec)), egui::StrokeKind::Inside);
    } else if enabled && resp.hovered() {
        p.rect_filled(rect, 4.0, HOVER_BG);
    }
    let ir = Rect::from_center_size(pos2(rect.center().x, rect.top() + 22.0), vec2(30.0, 30.0));
    icons::paint(p, ir, icon);
    if !enabled {
        p.rect_filled(ir.expand(2.0), 0.0, Color32::from_rgba_unmultiplied(244, 245, 247, 170));
    }
    p.text(pos2(rect.center().x, rect.bottom() - 9.0), egui::Align2::CENTER_CENTER, label, FontId::proportional(12.0), if enabled { TEXT } else { Color32::from_gray(160) });
    resp
}

fn ribbon_sep(ui: &mut Ui) {
    let (r, _) = ui.allocate_exact_size(vec2(12.0, 60.0), Sense::hover());
    ui.painter().line_segment([pos2(r.center().x, r.top() + 8.0), pos2(r.center().x, r.bottom() - 8.0)], Stroke::new(1.0, BORDER));
}
