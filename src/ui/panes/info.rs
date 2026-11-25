//! The information pane: properties (General) or DDL of the selection.

use eframe::egui::{self, vec2, FontId, RichText, Sense, Ui};

use crate::app::{Act, ConnState, ConnStatus, DbState, ObjMode, ObjRef, Workspace};
use crate::domain::DbKind;
use crate::shared::fmt_bytes;
use crate::ui::icons::{self, Icon};
use crate::ui::theme::{kind_color, pal};

const PG_OWNER_PREFIX: &str = "owner: ";

pub fn info(ui: &mut Ui, ws: &Workspace, info_ddl: &mut bool, acts: &mut Vec<Act>) {
    let table_sel = ws.sel.obj.is_some() && matches!(ws.mode, ObjMode::Tables | ObjMode::Views);
    tabs(ui, info_ddl, table_sel);
    let Some(c) = ws.sel.cid.and_then(|cid| ws.conn(cid)) else {
        ui.add_space(30.0);
        ui.vertical_centered(|ui| ui.label(RichText::new("No object selected").color(pal().muted)));
        return;
    };
    egui::ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
        egui::Frame::new().inner_margin(egui::Margin::symmetric(12, 10)).show(ui, |ui| body(ui, ws, c, *info_ddl, acts));
    });
}

fn tabs(ui: &mut Ui, info_ddl: &mut bool, table_sel: bool) {
    egui::Frame::new().fill(pal().header_bg).inner_margin(egui::Margin::symmetric(8, 4)).show(ui, |ui| {
        ui.horizontal(|ui| {
            if ui.selectable_label(!*info_ddl || !table_sel, "General").clicked() {
                *info_ddl = false;
            }
            if ui.add_enabled(table_sel, egui::Button::selectable(*info_ddl && table_sel, "DDL")).clicked() {
                *info_ddl = true;
            }
        });
    });
}

fn body(ui: &mut Ui, ws: &Workspace, c: &ConnState, show_ddl: bool, acts: &mut Vec<Act>) {
    let Some(dbn) = ws.sel.db.as_deref().filter(|_| ws.mode != ObjMode::Users) else {
        connection(ui, c);
        return;
    };
    let d = c.db(dbn);
    let Some(name) = ws.sel.obj.as_deref() else {
        header(ui, Icon::Database { open: true }, dbn, "Database");
        if let Some(d) = d {
            database(ui, d);
        }
        return;
    };
    let obj = ObjRef::new(c.cfg.id, dbn, name);
    match ws.mode {
        ObjMode::Tables | ObjMode::Views if show_ddl => ddl(ui, ws, d, &obj, acts),
        ObjMode::Tables | ObjMode::Views => table(ui, c, d, name),
        ObjMode::Functions => {
            let r = d.and_then(|d| d.routines.iter().find(|r| r.name == name));
            header(ui, Icon::Function, name, &r.map(|r| r.kind.clone()).unwrap_or_default());
            if let Some(r) = r {
                prop(ui, "Returns", &r.returns);
            }
        }
        ObjMode::Queries | ObjMode::Backups => saved_file(ui, ws.mode, d, name),
        ObjMode::Users => {}
    }
}

fn header(ui: &mut Ui, icon: Icon, title: &str, sub: &str) {
    ui.horizontal(|ui| {
        let (r, _) = ui.allocate_exact_size(vec2(40.0, 40.0), Sense::hover());
        icons::paint(ui.painter(), r, icon);
        ui.vertical(|ui| {
            ui.add(egui::Label::new(RichText::new(title).strong().size(15.0)).truncate());
            ui.label(RichText::new(sub).color(pal().muted));
        });
    });
    ui.add_space(12.0);
}

fn prop(ui: &mut Ui, k: &str, v: &str) {
    ui.label(RichText::new(k).color(pal().muted).size(12.0));
    ui.add(egui::Label::new(if v.is_empty() { "—" } else { v }).wrap());
    ui.add_space(7.0);
}

fn connection(ui: &mut Ui, c: &ConnState) {
    header(ui, Icon::Conn { color: kind_color(c.cfg.kind), open: c.is_open() }, &c.cfg.name, c.cfg.kind.label());
    let status = match &c.status {
        ConnStatus::Closed => "Closed".to_string(),
        ConnStatus::Opening => "Connecting…".to_string(),
        ConnStatus::Open => "Open".to_string(),
        ConnStatus::Failed(e) => format!("Failed: {}", e),
    };
    prop(ui, "Status", &status);
    if c.cfg.kind.is_builtin() {
        prop(ui, "Location", "Built into this app (changes last until you close it)");
    } else {
        prop(ui, "Host", &c.cfg.host);
        prop(ui, "Port", &c.cfg.port.to_string());
        prop(ui, "User Name", &c.cfg.user);
    }
    prop(ui, "Server Version", &c.version);
    prop(ui, "Databases", &if c.is_open() { c.dbs.len().to_string() } else { String::new() });
}

fn database(ui: &mut Ui, d: &DbState) {
    prop(ui, "Tables", &d.tables.iter().filter(|t| !t.is_view).count().to_string());
    prop(ui, "Views", &d.tables.iter().filter(|t| t.is_view).count().to_string());
    prop(ui, "Functions", &d.routines.len().to_string());
    prop(ui, "Data Length", &fmt_bytes(d.tables.iter().filter_map(|t| t.data_length).sum()));
    prop(ui, "Saved Queries", &d.queries.len().to_string());
    prop(ui, "Backups", &d.backups.len().to_string());
}

fn table(ui: &mut Ui, c: &ConnState, d: Option<&DbState>, name: &str) {
    let t = d.and_then(|d| d.tables.iter().find(|t| t.name == name));
    let is_view = t.is_some_and(|t| t.is_view);
    header(ui, if is_view { Icon::View } else { Icon::Table }, name, if is_view { "View" } else { "Table" });
    let Some(t) = t else { return };
    let pg = c.cfg.kind == DbKind::PostgreSql;
    if !is_view {
        prop(ui, "Rows", &t.rows.map(|r| r.to_string()).unwrap_or_default());
    }
    prop(ui, if pg { "Owner" } else { "Engine" }, t.engine.trim_start_matches(PG_OWNER_PREFIX));
    prop(ui, "Data Length", &t.data_length.map(fmt_bytes).unwrap_or_default());
    if !pg {
        prop(ui, "Created Date", &t.created);
        prop(ui, "Modified Date", &t.modified);
        prop(ui, "Collation", &t.collation);
    }
    prop(ui, "Comment", &t.comment);
}

fn ddl(ui: &mut Ui, ws: &Workspace, d: Option<&DbState>, obj: &ObjRef, acts: &mut Vec<Act>) {
    let key = (obj.cid, obj.db.clone(), obj.name.clone());
    match ws.ddl_cache.get(&key) {
        None => {
            ui.spinner();
            let is_view = d.and_then(|d| d.tables.iter().find(|t| t.name == obj.name)).is_some_and(|t| t.is_view);
            acts.push(Act::LoadDdl(obj.clone(), is_view));
        }
        Some(Ok(s)) if s.is_empty() => {
            ui.spinner();
        }
        Some(Ok(s)) => {
            let mut text = s.as_str();
            ui.add(egui::TextEdit::multiline(&mut text).font(FontId::monospace(12.0)).desired_width(f32::INFINITY).frame(egui::Frame::NONE));
            if ui.button("Copy DDL").clicked() {
                ui.ctx().copy_text(s.clone());
            }
        }
        Some(Err(e)) => {
            ui.colored_label(pal().err, e.to_string());
        }
    }
}

fn saved_file(ui: &mut Ui, mode: ObjMode, d: Option<&DbState>, name: &str) {
    let is_query = mode == ObjMode::Queries;
    header(ui, mode.icon(), name, if is_query { "Saved Query" } else { "Backup" });
    let Some(f) = d.and_then(|d| d.files(mode).iter().find(|f| f.name == name)) else { return };
    prop(ui, "Size", &fmt_bytes(f.size));
    prop(ui, "Modified Date", &f.modified);
    prop(ui, "File", &f.path.to_string_lossy());
    if is_query {
        ui.label(RichText::new(&f.preview).monospace().size(11.5));
    }
}
