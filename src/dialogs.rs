//! Pop-up windows: alerts, confirmations, wizards and the connection editor.

use eframe::egui::{self, vec2, Color32, FontId, RichText, Sense, Ui};

use crate::app::*;
use crate::backend::{self, ExportFormat, Purpose, Req};
use crate::icons::{self, Icon};
use crate::model::*;
use crate::panes::ERR;
use crate::sqlgen::quote_ident;
use crate::views::MUTED;

fn win(title: &str) -> egui::Window<'static> {
    egui::Window::new(title.to_string()).collapsible(false).resizable(false).anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
}

fn buttons(ui: &mut Ui, ok_label: &str, ok_enabled: bool) -> (bool, bool) {
    let (mut ok, mut cancel) = (false, false);
    ui.add_space(8.0);
    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
        if ui.button("Cancel").clicked() || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
            cancel = true;
        }
        if ui.add_enabled(ok_enabled, egui::Button::new(RichText::new(format!("  {}  ", ok_label)).strong())).clicked() {
            ok = true;
        }
    });
    (ok, cancel)
}

fn file_row(ui: &mut Ui, path: &mut String, hint: &str, pick: impl FnOnce() -> Option<std::path::PathBuf>) {
    ui.horizontal(|ui| {
        ui.add(egui::TextEdit::singleline(path).desired_width(340.0).hint_text(hint));
        if ui.button("…").clicked() {
            if let Some(p) = pick() {
                *path = p.to_string_lossy().to_string();
            }
        }
    });
}

impl Dbz {
    pub fn dialogs(&mut self, ctx: &egui::Context, acts: &mut Vec<Act>) {
        if let Some((title, msg)) = self.alert.clone() {
            let mut close = false;
            win(&title).show(ctx, |ui| {
                ui.set_max_width(460.0);
                egui::ScrollArea::vertical().max_height(300.0).show(ui, |ui| {
                    ui.add(egui::Label::new(msg.clone()).wrap().selectable(true));
                });
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if ui.button("Copy").clicked() {
                        ui.ctx().copy_text(msg.clone());
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("   OK   ").clicked() || ui.input(|i| i.key_pressed(egui::Key::Enter) || i.key_pressed(egui::Key::Escape)) {
                            close = true;
                        }
                    });
                });
            });
            if close {
                self.alert = None;
            }
            return;
        }
        self.conn_dialog_ui(ctx);
        let Some(mut dialog) = self.dialog.take() else { return };
        let mut keep = true;
        let mut confirmed = false;
        let mut save_query: Option<(u64, String)> = None;
        match &mut dialog {
            Dialog::Confirm { title, msg, .. } => {
                win(title).show(ctx, |ui| {
                    ui.set_max_width(420.0);
                    ui.horizontal(|ui| {
                        let (r, _) = ui.allocate_exact_size(vec2(32.0, 32.0), Sense::hover());
                        ui.painter().circle_filled(r.center(), 14.0, Color32::from_rgb(0xf0, 0xa0, 0x20));
                        ui.painter().text(r.center(), egui::Align2::CENTER_CENTER, "!", FontId::proportional(18.0), Color32::WHITE);
                        ui.add(egui::Label::new(msg.as_str()).wrap());
                    });
                    let (ok, cancel) = buttons(ui, "OK", true);
                    if ok {
                        confirmed = true;
                        keep = false;
                    }
                    if cancel {
                        keep = false;
                    }
                });
            }
            Dialog::NewDb { cid, name, charset } => {
                let kind = self.conn(*cid).map(|c| c.cfg.kind).unwrap_or(DbKind::Sample);
                let pg = kind.is_pg();
                win("New Database").show(ctx, |ui| {
                    egui::Grid::new("newdb").num_columns(2).spacing([10.0, 8.0]).show(ui, |ui| {
                        ui.label("Database Name:");
                        ui.add(egui::TextEdit::singleline(name).desired_width(240.0));
                        ui.end_row();
                        if kind != DbKind::Sample {
                            ui.label(if pg { "Encoding:" } else { "Character Set:" });
                            ui.add(egui::TextEdit::singleline(charset).desired_width(240.0));
                            ui.end_row();
                        }
                    });
                    let (ok, cancel) = buttons(ui, "OK", !name.trim().is_empty());
                    if ok {
                        let q = quote_ident(pg, name.trim());
                        let cs = charset.trim().replace(['\'', ';', ' '], "");
                        let sql = if cs.is_empty() || kind == DbKind::Sample {
                            format!("CREATE DATABASE {}", q)
                        } else if pg {
                            format!("CREATE DATABASE {} ENCODING '{}'", q, cs)
                        } else {
                            format!("CREATE DATABASE {} CHARACTER SET {}", q, cs)
                        };
                        acts.push(Act::Send(*cid, Req::Exec { tab: 0, db: String::new(), stmts: vec![sql], purpose: Purpose::Other("database".into()) }));
                    }
                    keep = !(ok || cancel);
                });
            }
            Dialog::Export(st) => {
                win("Export Wizard").show(ctx, |ui| {
                    ui.label(RichText::new(format!("Export the data of \"{}\" to a file.", st.table)).strong());
                    ui.add_space(6.0);
                    ui.label("Format:");
                    for f in ExportFormat::ALL {
                        if ui.radio_value(&mut st.format, f, f.label()).changed() {
                            st.path = std::path::Path::new(&st.path).with_extension(f.ext()).to_string_lossy().to_string();
                        }
                    }
                    ui.add_space(6.0);
                    ui.checkbox(&mut st.header, "Include column titles");
                    ui.add_space(6.0);
                    ui.label("Save to:");
                    let (label, ext) = (st.format.label(), st.format.ext());
                    let fname = std::path::Path::new(&st.path).file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
                    file_row(ui, &mut st.path, "", || rfd::FileDialog::new().add_filter(label, &[ext]).set_file_name(fname).save_file());
                    let (ok, cancel) = buttons(ui, "Export", !st.path.trim().is_empty());
                    if ok {
                        acts.push(Act::Status(format!("Exporting {}…", st.table)));
                        acts.push(Act::Send(st.cid, Req::Export { db: st.db.clone(), table: st.table.clone(), format: st.format, path: st.path.clone(), header: st.header }));
                    }
                    keep = !(ok || cancel);
                });
            }
            Dialog::Import { cid, db, table, path, header, delim, create } => {
                let tables: Vec<String> = self.conn(*cid).and_then(|c| c.db(db)).map(|d| d.tables.iter().filter(|t| !t.is_view).map(|t| t.name.clone()).collect()).unwrap_or_default();
                win("Import Wizard").show(ctx, |ui| {
                    ui.label(RichText::new(format!("Import a CSV or text file into \"{}\".", db)).strong());
                    ui.add_space(6.0);
                    ui.label("Source file:");
                    let before = path.clone();
                    file_row(ui, path, "Choose a file…", || rfd::FileDialog::new().add_filter("CSV / text", &["csv", "txt", "tsv"]).pick_file());
                    if *path != before {
                        let p = std::path::Path::new(path.as_str());
                        if *create && table.is_empty() {
                            *table = p.file_stem().map(|s| sanitize(&s.to_string_lossy()).replace(' ', "_")).unwrap_or_default();
                        }
                        if p.extension().map(|e| e == "tsv" || e == "txt").unwrap_or(false) {
                            *delim = '\t';
                        }
                    }
                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        ui.label("Field separator:");
                        ui.radio_value(delim, ',', "Comma");
                        ui.radio_value(delim, ';', "Semicolon");
                        ui.radio_value(delim, '\t', "Tab");
                    });
                    ui.checkbox(header, "First row holds the field names");
                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        ui.radio_value(create, false, "Into an existing table");
                        ui.radio_value(create, true, "Into a new table");
                    });
                    if *create {
                        ui.horizontal(|ui| {
                            ui.label("New table name:");
                            ui.add(egui::TextEdit::singleline(table).desired_width(220.0));
                        });
                    } else {
                        egui::ComboBox::from_label("Target table").selected_text(if table.is_empty() { "(choose)".to_string() } else { table.clone() }).show_ui(ui, |ui| {
                            for t in &tables {
                                ui.selectable_value(table, t.clone(), t);
                            }
                        });
                    }
                    let (ok, cancel) = buttons(ui, "Import", !path.trim().is_empty() && !table.trim().is_empty());
                    if ok {
                        acts.push(Act::Status(format!("Importing into {}…", table)));
                        acts.push(Act::Send(*cid, Req::Import { db: db.clone(), table: table.trim().to_string(), path: path.clone(), header: *header, delim: *delim, create: *create }));
                    }
                    keep = !(ok || cancel);
                });
            }
            Dialog::RunFile { cid, db, path, cont } => {
                win("Execute SQL File").show(ctx, |ui| {
                    ui.label(format!("Run every statement of a .sql file in \"{}\".", db));
                    ui.add_space(6.0);
                    file_row(ui, path, "Choose a file…", || rfd::FileDialog::new().add_filter("SQL file", &["sql", "txt"]).pick_file());
                    ui.checkbox(cont, "Continue on error");
                    let (ok, cancel) = buttons(ui, "Start", !path.trim().is_empty());
                    if ok {
                        acts.push(Act::Status("Running SQL file…".into()));
                        acts.push(Act::Send(*cid, Req::RunFile { db: db.clone(), path: path.clone(), continue_on_error: *cont }));
                    }
                    keep = !(ok || cancel);
                });
            }
            Dialog::SaveQuery { tab, name } => {
                win("Save Query").show(ctx, |ui| {
                    ui.label("Query name:");
                    ui.add(egui::TextEdit::singleline(name).desired_width(280.0));
                    let (ok, cancel) = buttons(ui, "Save", !name.trim().is_empty());
                    if ok {
                        save_query = Some((*tab, name.trim().to_string()));
                    }
                    keep = !(ok || cancel);
                });
            }
            Dialog::Rename { cid, db, table, new } => {
                let pg = self.conn(*cid).map(|c| c.cfg.kind.is_pg()).unwrap_or(false);
                win("Rename Table").show(ctx, |ui| {
                    ui.label(format!("New name for \"{}\":", table));
                    ui.add(egui::TextEdit::singleline(new).desired_width(280.0));
                    let (ok, cancel) = buttons(ui, "OK", !new.trim().is_empty() && new.trim() != table);
                    if ok {
                        let target = if pg { new.trim().rsplit('.').next().unwrap_or("").to_string() } else { new.trim().to_string() };
                        let sql = format!("ALTER TABLE {} RENAME TO {}", quote_ident(pg, table), quote_ident(pg, &target));
                        acts.push(Act::Send(*cid, Req::Exec { tab: 0, db: db.clone(), stmts: vec![sql], purpose: Purpose::Refresh }));
                    }
                    keep = !(ok || cancel);
                });
            }
            Dialog::About => {
                win("About DBZ").show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        icons::logo(ui, 56.0);
                        ui.vertical(|ui| {
                            ui.label(RichText::new("DBZ").strong().size(18.0));
                            ui.label("Version 0.1 — a database manager written in Rust");
                        });
                    });
                    ui.add_space(6.0);
                    ui.label("Connects to MySQL, MariaDB and PostgreSQL, and includes a built-in sample database to try things out.");
                    ui.add_space(8.0);
                    if ui.button("   OK   ").clicked() {
                        keep = false;
                    }
                });
            }
        }
        if confirmed {
            if let Dialog::Confirm { action, .. } = dialog {
                self.run_confirm(action);
            }
            return;
        }
        if let Some((tab, name)) = save_query {
            self.write_query(tab, name);
        }
        if keep {
            self.dialog = Some(dialog);
        }
    }

    fn conn_dialog_ui(&mut self, ctx: &egui::Context) {
        let Some(mut d) = self.conn_dialog.take() else { return };
        let mut keep = true;
        let mut save = false;
        let mut test = false;
        let title = if d.editing.is_some() { "Edit Connection" } else { "New Connection" };
        win(title).show(ctx, |ui| {
            ui.set_width(500.0);
            ui.horizontal(|ui| {
                for (i, l) in ["General", "Advanced"].iter().enumerate() {
                    if ui.selectable_label(d.page == i, *l).clicked() {
                        d.page = i;
                    }
                }
            });
            ui.separator();
            ui.horizontal(|ui| {
                let (r, _) = ui.allocate_exact_size(vec2(44.0, 44.0), Sense::hover());
                icons::paint(ui.painter(), r, Icon::Conn { color: d.cfg.kind.color(), open: true });
                ui.add_space(8.0);
                ui.label(RichText::new(d.cfg.kind.label()).size(16.0).strong());
            });
            ui.add_space(8.0);
            let sample = d.cfg.kind == DbKind::Sample;
            egui::Grid::new("conn_grid").num_columns(2).spacing([12.0, 8.0]).min_col_width(130.0).show(ui, |ui| {
                if d.page == 0 {
                    ui.label("Connection Name:");
                    ui.add(egui::TextEdit::singleline(&mut d.cfg.name).desired_width(300.0).hint_text("e.g. Local MySQL"));
                    ui.end_row();
                    ui.label("Type:");
                    let mut kind = d.cfg.kind;
                    egui::ComboBox::from_id_salt("kind").selected_text(kind.label()).width(300.0).show_ui(ui, |ui| {
                        for k in DbKind::ALL {
                            ui.selectable_value(&mut kind, k, k.label());
                        }
                    });
                    if kind != d.cfg.kind {
                        if d.cfg.port == d.cfg.kind.default_port() || d.cfg.port == 0 {
                            d.cfg.port = kind.default_port();
                        }
                        if d.cfg.user.is_empty() || d.cfg.user == d.cfg.kind.default_user() || d.cfg.user == "sample" {
                            d.cfg.user = kind.default_user().into();
                        }
                        if kind != DbKind::Sample && (d.cfg.host.is_empty() || d.cfg.host == "built-in") {
                            d.cfg.host = "localhost".into();
                        }
                        d.cfg.kind = kind;
                    }
                    ui.end_row();
                    if !sample {
                        ui.label("Host:");
                        ui.add(egui::TextEdit::singleline(&mut d.cfg.host).desired_width(300.0));
                        ui.end_row();
                        ui.label("Port:");
                        let mut port = d.cfg.port.to_string();
                        if ui.add(egui::TextEdit::singleline(&mut port).desired_width(80.0)).changed() {
                            d.cfg.port = port.trim().parse().unwrap_or(d.cfg.port);
                        }
                        ui.end_row();
                        ui.label("User Name:");
                        ui.add(egui::TextEdit::singleline(&mut d.cfg.user).desired_width(300.0));
                        ui.end_row();
                        ui.label("Password:");
                        ui.add(egui::TextEdit::singleline(&mut d.cfg.password).password(true).desired_width(300.0));
                        ui.end_row();
                        ui.label("");
                        ui.checkbox(&mut d.cfg.save_password, "Save password");
                        ui.end_row();
                    } else {
                        ui.label("");
                        ui.add(egui::Label::new(RichText::new("A small database built into the app, with shop and analytics sample tables. Changes last until the app closes.").color(MUTED)).wrap());
                        ui.end_row();
                    }
                } else {
                    ui.label("Initial Database:");
                    ui.add(egui::TextEdit::singleline(&mut d.cfg.initial_db).desired_width(300.0).hint_text(if d.cfg.kind.is_pg() { "postgres" } else { "(none)" }));
                    ui.end_row();
                    ui.label("Connect Timeout (s):");
                    let mut t = d.cfg.connect_timeout.to_string();
                    if ui.add(egui::TextEdit::singleline(&mut t).desired_width(80.0)).changed() {
                        d.cfg.connect_timeout = t.trim().parse().unwrap_or(d.cfg.connect_timeout);
                    }
                    ui.end_row();
                }
            });
            if !d.cfg.save_password && !sample {
                ui.label(RichText::new("It will be kept only until the app closes.").color(MUTED).size(12.0));
            }
            ui.add_space(8.0);
            match &d.test {
                Some(Ok(m)) => {
                    ui.colored_label(Color32::from_rgb(0x2a, 0x8a, 0x4a), format!("Success: {}", m));
                }
                Some(Err(e)) => {
                    ui.add(egui::Label::new(RichText::new(format!("Failed: {}", e)).color(ERR)).wrap());
                }
                None => {
                    if d.test_id.is_some() {
                        ui.horizontal(|ui| {
                            ui.spinner();
                            ui.label("Testing…");
                        });
                    }
                }
            }
            ui.separator();
            ui.horizontal(|ui| {
                if ui.button("Test Connection").clicked() {
                    test = true;
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("Cancel").clicked() {
                        keep = false;
                    }
                    if ui.button(RichText::new("   OK   ").strong()).clicked() {
                        save = true;
                    }
                });
            });
        });
        if test {
            self.next_tab += 1;
            let id = u64::MAX - self.next_tab;
            let mut cfg = d.cfg.clone();
            cfg.id = id;
            let tx = backend::spawn(cfg, self.sample.clone(), self.tx.clone(), self.ctx.clone());
            let _ = tx.send(Req::Connect);
            d.test = None;
            d.test_id = Some(id);
            self.test_tx = Some((id, tx));
        }
        if save {
            if d.cfg.name.trim().is_empty() {
                d.cfg.name = if d.cfg.kind == DbKind::Sample { "Sample Data".into() } else { format!("{}@{}", d.cfg.user, d.cfg.host) };
            }
            match d.editing {
                Some(id) => {
                    let reopen = self.conn(id).map(|c| c.tx.is_some()).unwrap_or(false);
                    if reopen {
                        self.do_act(Act::CloseConn(id));
                    }
                    if let Some(c) = self.conn_mut(id) {
                        c.cfg = d.cfg.clone();
                        c.status = ConnStatus::Closed;
                    }
                    if reopen {
                        self.do_act(Act::OpenConn(id));
                    }
                }
                None => {
                    d.cfg.id = self.next_conn;
                    self.next_conn += 1;
                    let id = d.cfg.id;
                    self.conns.push(ConnState::new(d.cfg.clone()));
                    self.do_act(Act::OpenConn(id));
                }
            }
            self.save_config();
            keep = false;
        }
        if keep {
            self.conn_dialog = Some(d);
        } else {
            self.test_tx = None;
        }
    }
}
