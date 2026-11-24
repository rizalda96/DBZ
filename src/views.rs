//! The work tabs: SQL query editor, table data viewer/editor and table designer.

use std::time::Duration;

use eframe::egui::{self, pos2, vec2, Color32, FontId, Rect, Response, RichText, Sense, Stroke, Ui};
use egui_extras::{Column, TableBuilder};

use crate::app::{Act, ConnState};
use crate::backend::{ExportFormat, Purpose, Req};
use crate::icons::{self, Icon};
use crate::model::*;
use crate::sqlgen;

pub const ACCENT: Color32 = Color32::from_rgb(0x1f, 0x6f, 0xd1);
pub const SEL_BG: Color32 = Color32::from_rgb(0xcc, 0xe2, 0xfa);
pub const HOVER_BG: Color32 = Color32::from_rgb(0xe6, 0xef, 0xfa);
pub const HEADER_BG: Color32 = Color32::from_rgb(0xf0, 0xf1, 0xf3);
pub const TOOLBAR_BG: Color32 = Color32::from_rgb(0xf7, 0xf7, 0xf8);
pub const BORDER: Color32 = Color32::from_rgb(0xd4, 0xd7, 0xdc);
pub const GRID_LINE: Color32 = Color32::from_rgb(0xe3, 0xe5, 0xe8);
pub const MUTED: Color32 = Color32::from_rgb(0x8a, 0x90, 0x99);
pub const TEXT: Color32 = Color32::from_rgb(0x22, 0x26, 0x2b);
pub const EDITED_BG: Color32 = Color32::from_rgb(0xff, 0xf6, 0xd5);
pub const NEW_BG: Color32 = Color32::from_rgb(0xe4, 0xf6, 0xe7);
pub const DELETED_BG: Color32 = Color32::from_rgb(0xfb, 0xe3, 0xe3);

// ---------------------------------------------------------------- shared widgets

pub fn tool_btn(ui: &mut Ui, icon: Option<Icon>, text: &str, enabled: bool) -> Response {
    let color = if enabled { TEXT } else { Color32::from_gray(170) };
    let galley = ui.painter().layout_no_wrap(text.to_owned(), FontId::proportional(12.5), color);
    let w = galley.size().x + if icon.is_some() { 30.0 } else { 14.0 };
    let (rect, resp) = ui.allocate_exact_size(vec2(w, 26.0), if enabled { Sense::click() } else { Sense::hover() });
    let p = ui.painter();
    if enabled && resp.hovered() {
        p.rect_filled(rect, 3.0, HOVER_BG);
        p.rect_stroke(rect, 3.0, Stroke::new(1.0, Color32::from_rgb(0xb5, 0xcf, 0xef)), egui::StrokeKind::Inside);
    }
    let mut x = rect.left() + 7.0;
    if let Some(i) = icon {
        let ir = Rect::from_center_size(pos2(x + 8.0, rect.center().y), vec2(16.0, 16.0));
        icons::paint(p, ir, i);
        if !enabled {
            p.rect_filled(ir.expand(1.0), 0.0, Color32::from_rgba_unmultiplied(247, 247, 248, 170));
        }
        x += 21.0;
    }
    p.galley(pos2(x, rect.center().y - galley.size().y / 2.0), galley, color);
    resp
}

/// Small filled triangle. dir: 0 right, 1 down, 2 left, 3 up.
pub fn arrow(p: &egui::Painter, c: egui::Pos2, r: f32, dir: u8, color: Color32) {
    let pts = match dir {
        0 => vec![pos2(c.x - r * 0.6, c.y - r), pos2(c.x + r * 0.8, c.y), pos2(c.x - r * 0.6, c.y + r)],
        1 => vec![pos2(c.x - r, c.y - r * 0.6), pos2(c.x + r, c.y - r * 0.6), pos2(c.x, c.y + r * 0.8)],
        2 => vec![pos2(c.x + r * 0.6, c.y - r), pos2(c.x - r * 0.8, c.y), pos2(c.x + r * 0.6, c.y + r)],
        _ => vec![pos2(c.x - r, c.y + r * 0.6), pos2(c.x + r, c.y + r * 0.6), pos2(c.x, c.y - r * 0.8)],
    };
    p.add(egui::Shape::convex_polygon(pts, color, Stroke::NONE));
}

pub fn toolbar_frame() -> egui::Frame {
    egui::Frame::new().fill(TOOLBAR_BG).inner_margin(egui::Margin::symmetric(6, 3))
}

fn sep(ui: &mut Ui) {
    let (r, _) = ui.allocate_exact_size(vec2(9.0, 22.0), Sense::hover());
    ui.painter().line_segment([pos2(r.center().x, r.top() + 3.0), pos2(r.center().x, r.bottom() - 3.0)], Stroke::new(1.0, BORDER));
}

fn cell_lines(ui: &Ui, rect: Rect) {
    let p = ui.painter();
    p.line_segment([rect.right_top(), rect.right_bottom()], Stroke::new(1.0, GRID_LINE));
    p.line_segment([rect.left_bottom(), rect.right_bottom()], Stroke::new(1.0, GRID_LINE));
}

fn cell_text(ui: &mut Ui, v: &Cell, color: Color32) {
    ui.add_space(5.0);
    match v {
        None => {
            ui.add(egui::Label::new(RichText::new("(Null)").italics().color(MUTED)).truncate().selectable(false));
        }
        Some(s) => {
            let one_line: String = if s.contains('\n') || s.len() > 300 { s.chars().take(300).map(|c| if c == '\n' || c == '\r' { ' ' } else { c }).collect() } else { s.clone() };
            ui.add(egui::Label::new(RichText::new(one_line).color(color)).truncate().selectable(false));
        }
    }
}

fn col_width(name: &str, rows: &[Row], idx: usize) -> f32 {
    let mut w = name.chars().count() as f32 * 7.5 + 30.0;
    for r in rows.iter().take(50) {
        if let Some(Some(v)) = r.get(idx) {
            w = w.max(v.chars().count().min(60) as f32 * 7.0 + 16.0);
        }
    }
    w.clamp(60.0, 360.0)
}

/// Read-only result grid used by the query editor and the Users list.
pub fn result_grid(ui: &mut Ui, id: impl std::hash::Hash + std::fmt::Debug, columns: &[String], rows: &[Row], sel: &mut Option<(usize, usize)>) {
    if columns.is_empty() {
        ui.add_space(8.0);
        ui.label(RichText::new("  No result set").color(MUTED));
        return;
    }
    ui.push_id(id, |ui| {
        ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
        let mut tb = TableBuilder::new(ui).striped(true).sense(Sense::click()).resizable(true).auto_shrink(false).cell_layout(egui::Layout::left_to_right(egui::Align::Center)).column(Column::exact(36.0));
        for (i, c) in columns.iter().enumerate() {
            tb = tb.column(Column::initial(col_width(c, rows, i)).at_least(40.0).clip(true));
        }
        tb.header(24.0, |mut h| {
            h.col(|ui| {
                ui.painter().rect_filled(ui.max_rect(), 0.0, HEADER_BG);
                cell_lines(ui, ui.max_rect());
            });
            for c in columns {
                h.col(|ui| {
                    ui.painter().rect_filled(ui.max_rect(), 0.0, HEADER_BG);
                    cell_lines(ui, ui.max_rect());
                    ui.add_space(5.0);
                    ui.add(egui::Label::new(RichText::new(c).strong()).truncate().selectable(false));
                });
            }
        })
        .body(|body| {
            body.rows(22.0, rows.len(), |mut row| {
                let r = row.index();
                row.col(|ui| {
                    let rect = ui.max_rect();
                    ui.painter().rect_filled(rect, 0.0, HEADER_BG);
                    cell_lines(ui, rect);
                    if matches!(sel, Some((sr, _)) if *sr == r) {
                        arrow(ui.painter(), pos2(rect.left() + 13.0, rect.center().y), 4.0, 0, ACCENT);
                    }
                });
                for c in 0..columns.len() {
                    let (rect, resp) = row.col(|ui| {
                        let rect = ui.max_rect();
                        if *sel == Some((r, c)) {
                            ui.painter().rect_filled(rect, 0.0, SEL_BG);
                        }
                        cell_lines(ui, rect);
                        cell_text(ui, rows[r].get(c).unwrap_or(&None), TEXT);
                    });
                    let _ = rect;
                    if resp.clicked() || resp.secondary_clicked() {
                        *sel = Some((r, c));
                    }
                    resp.context_menu(|ui| {
                        if ui.button("Copy").clicked() {
                            ui.ctx().copy_text(rows[r].get(c).cloned().flatten().unwrap_or_default());
                            ui.close();
                        }
                        if ui.button("Copy Row (tab separated)").clicked() {
                            ui.ctx().copy_text(rows[r].iter().map(|v| v.clone().unwrap_or_default()).collect::<Vec<_>>().join("\t"));
                            ui.close();
                        }
                        if ui.button("Copy Column Name").clicked() {
                            ui.ctx().copy_text(columns[c].clone());
                            ui.close();
                        }
                    });
                }
            });
        });
    });
    if let Some((r, c)) = *sel {
        if ui.input(|i| i.modifiers.command && i.key_pressed(egui::Key::C)) && ui.ctx().memory(|m| m.focused().is_none()) {
            if let Some(v) = rows.get(r).and_then(|row| row.get(c)) {
                ui.ctx().copy_text(v.clone().unwrap_or_default());
            }
        }
    }
}

// ---------------------------------------------------------------- query tab

pub struct QueryTab {
    pub id: u64,
    pub cid: u64,
    pub db: String,
    pub title: String,
    pub sql: String,
    pub saved_name: Option<String>,
    pub results: Vec<ResultSet>,
    pub error: Option<(String, String)>,
    pub running: bool,
    pub elapsed: Option<Duration>,
    pub view: usize,
    pub sel_cell: Option<(usize, usize)>,
    pub dirty: bool,
    pub selection: Option<(usize, usize)>,
    pub kind: DbKind,
}

const KEYWORDS: &[&str] = &[
    "SELECT", "FROM", "WHERE", "AND", "OR", "NOT", "NULL", "IS", "IN", "LIKE", "BETWEEN", "ORDER", "BY", "GROUP", "HAVING", "LIMIT", "OFFSET", "INSERT", "INTO", "VALUES", "UPDATE", "SET", "DELETE", "CREATE", "TABLE", "VIEW", "INDEX", "DROP",
    "ALTER", "ADD", "COLUMN", "PRIMARY", "KEY", "FOREIGN", "REFERENCES", "DEFAULT", "AUTO_INCREMENT", "UNIQUE", "JOIN", "LEFT", "RIGHT", "INNER", "OUTER", "FULL", "CROSS", "ON", "AS", "DISTINCT", "UNION", "ALL", "CASE", "WHEN", "THEN",
    "ELSE", "END", "ASC", "DESC", "DATABASE", "SCHEMA", "USE", "SHOW", "TABLES", "DATABASES", "COLUMNS", "IF", "EXISTS", "TRUNCATE", "BEGIN", "COMMIT", "ROLLBACK", "TRANSACTION", "EXPLAIN", "WITH", "RETURNING", "COUNT", "SUM", "AVG",
    "MIN", "MAX", "TRUE", "FALSE", "PROCEDURE", "FUNCTION", "RETURNS", "RETURN", "DECLARE", "CALL", "GRANT", "REVOKE", "CHANGE", "MODIFY", "RENAME", "TO", "COMMENT", "ENGINE", "CHARSET", "DESCRIBE", "REPLACE", "OVER", "PARTITION",
];

fn highlight(text: &str) -> egui::text::LayoutJob {
    use egui::text::{LayoutJob, TextFormat};
    let mut job = LayoutJob::default();
    let font = FontId::monospace(13.5);
    let fmt = |c: Color32| TextFormat { font_id: font.clone(), color: c, ..Default::default() };
    let kw = Color32::from_rgb(0x1a, 0x4f, 0xc4);
    let string = Color32::from_rgb(0xb8, 0x3a, 0x26);
    let num = Color32::from_rgb(0x0a, 0x86, 0x6e);
    let comment = Color32::from_rgb(0x6a, 0x9a, 0x55);
    let ident = Color32::from_rgb(0x8a, 0x3f, 0x9e);
    let chars: Vec<(usize, char)> = text.char_indices().collect();
    let mut i = 0;
    while i < chars.len() {
        let (start, c) = chars[i];
        let mut j = i + 1;
        let color;
        if c == '-' && chars.get(i + 1).map(|x| x.1) == Some('-') || c == '#' {
            while j < chars.len() && chars[j].1 != '\n' {
                j += 1;
            }
            color = comment;
        } else if c == '/' && chars.get(i + 1).map(|x| x.1) == Some('*') {
            j = i + 2;
            while j < chars.len() && !(chars[j].1 == '/' && chars[j - 1].1 == '*') {
                j += 1;
            }
            j = (j + 1).min(chars.len());
            color = comment;
        } else if c == '\'' || c == '"' || c == '`' {
            while j < chars.len() && chars[j].1 != c {
                j += 1;
            }
            j = (j + 1).min(chars.len());
            color = if c == '\'' { string } else { ident };
        } else if c.is_ascii_digit() {
            while j < chars.len() && (chars[j].1.is_ascii_digit() || chars[j].1 == '.') {
                j += 1;
            }
            color = num;
        } else if c.is_alphabetic() || c == '_' {
            while j < chars.len() && (chars[j].1.is_alphanumeric() || chars[j].1 == '_') {
                j += 1;
            }
            let end = chars.get(j).map(|x| x.0).unwrap_or(text.len());
            let word = text[start..end].to_uppercase();
            color = if KEYWORDS.contains(&word.as_str()) { kw } else { TEXT };
        } else {
            color = TEXT;
        }
        let end = chars.get(j).map(|x| x.0).unwrap_or(text.len());
        job.append(&text[start..end], 0.0, fmt(color));
        i = j;
    }
    job
}

pub fn beautify(sql: &str) -> String {
    let breaks = ["FROM", "WHERE", "ORDER BY", "GROUP BY", "HAVING", "LIMIT", "LEFT JOIN", "RIGHT JOIN", "INNER JOIN", "JOIN", "VALUES", "SET", "UNION"];
    let mut out = Vec::new();
    for stmt in sqlgen::split_statements(sql) {
        // upper-case keywords outside quotes
        let mut s = String::new();
        let mut word = String::new();
        let mut quote: Option<char> = None;
        let flush = |w: &mut String, s: &mut String| {
            if KEYWORDS.contains(&w.to_uppercase().as_str()) {
                s.push_str(&w.to_uppercase());
            } else {
                s.push_str(w);
            }
            w.clear();
        };
        for c in stmt.chars() {
            if let Some(q) = quote {
                s.push(c);
                if c == q {
                    quote = None;
                }
                continue;
            }
            if c == '\'' || c == '"' || c == '`' {
                flush(&mut word, &mut s);
                quote = Some(c);
                s.push(c);
            } else if c.is_alphanumeric() || c == '_' {
                word.push(c);
            } else {
                flush(&mut word, &mut s);
                s.push(c);
            }
        }
        flush(&mut word, &mut s);
        let mut s = s.split_whitespace().collect::<Vec<_>>().join(" ");
        for b in breaks {
            s = s.replace(&format!(" {} ", b), &format!("\n{} ", b));
        }
        s = s.replace(" AND ", "\n  AND ").replace(" OR ", "\n  OR ");
        out.push(format!("{};", s));
    }
    out.join("\n\n")
}

pub fn query_view(ui: &mut Ui, t: &mut QueryTab, conns: &[ConnState], acts: &mut Vec<Act>) {
    let run_shortcut = ui.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::R) || i.consume_key(egui::Modifiers::NONE, egui::Key::F9));
    let save_shortcut = ui.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::S));
    let mut run = run_shortcut;
    let mut explain = false;
    toolbar_frame().show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 2.0;
            if tool_btn(ui, Some(Icon::Save), "Save", true).clicked() || save_shortcut {
                acts.push(Act::SaveQuery(t.id));
            }
            sep(ui);
            let cname = conns.iter().find(|c| c.cfg.id == t.cid).map(|c| c.cfg.name.clone()).unwrap_or_else(|| "(none)".into());
            egui::ComboBox::from_id_salt(("qconn", t.id)).selected_text(cname).width(150.0).show_ui(ui, |ui| {
                for c in conns {
                    if ui.selectable_label(c.cfg.id == t.cid, &c.cfg.name).clicked() {
                        t.cid = c.cfg.id;
                        t.kind = c.cfg.kind;
                        t.db = c.dbs.first().map(|d| d.name.clone()).unwrap_or_default();
                        acts.push(Act::EnsureOpen(c.cfg.id));
                    }
                }
            });
            let dbs: Vec<String> = conns.iter().find(|c| c.cfg.id == t.cid).map(|c| c.dbs.iter().map(|d| d.name.clone()).collect()).unwrap_or_default();
            egui::ComboBox::from_id_salt(("qdb", t.id)).selected_text(if t.db.is_empty() { "(database)".to_string() } else { t.db.clone() }).width(140.0).show_ui(ui, |ui| {
                for d in dbs {
                    if ui.selectable_label(d == t.db, &d).clicked() {
                        t.db = d;
                    }
                }
            });
            sep(ui);
            let has_sel = t.selection.map(|(a, b)| a != b).unwrap_or(false);
            if tool_btn(ui, Some(Icon::Run), if has_sel { "Run Selected" } else { "Run" }, !t.running).on_hover_text("Ctrl+R").clicked() {
                run = true;
            }
            if tool_btn(ui, None, "Stop", false).clicked() {}
            if tool_btn(ui, None, "Explain", !t.running).clicked() {
                explain = true;
            }
            sep(ui);
            if tool_btn(ui, None, "Beautify SQL", true).clicked() {
                t.sql = beautify(&t.sql);
                t.dirty = true;
            }
            if tool_btn(ui, None, "Clear", true).clicked() {
                t.sql.clear();
                t.dirty = true;
            }
            if t.running {
                ui.add_space(8.0);
                ui.spinner();
                ui.label("Running…");
            }
        });
    });
    if (run || explain) && !t.running {
        let mut sql = t.sql.clone();
        if let Some((a, b)) = t.selection {
            if a != b {
                sql = t.sql.chars().skip(a.min(b)).take(a.max(b) - a.min(b)).collect();
            }
        }
        if explain {
            sql = sqlgen::split_statements(&sql).into_iter().map(|s| format!("EXPLAIN {}", s)).collect::<Vec<_>>().join(";\n");
        }
        if sql.trim().is_empty() {
            acts.push(Act::Status("Nothing to run".into()));
        } else {
            t.running = true;
            t.error = None;
            acts.push(Act::Send(t.cid, Req::Query { tab: t.id, db: t.db.clone(), sql }));
        }
    }

    egui::Panel::bottom(("qres", t.id)).resizable(true).default_size(280.0).min_size(80.0).frame(egui::Frame::new().fill(Color32::WHITE).stroke(Stroke::new(1.0, BORDER))).show(ui, |ui| {
        // result tabs
        egui::Frame::new().fill(HEADER_BG).inner_margin(egui::Margin::symmetric(4, 2)).show(ui, |ui| {
            ui.horizontal(|ui| {
                if ui.selectable_label(t.view == 0, "Message").clicked() {
                    t.view = 0;
                }
                let mut n = 0;
                for (i, r) in t.results.iter().enumerate() {
                    if r.has_rows {
                        n += 1;
                        if ui.selectable_label(t.view == i + 1, format!("Result {}", n)).clicked() {
                            t.view = i + 1;
                            t.sel_cell = None;
                        }
                    }
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if let Some(e) = t.elapsed {
                        let rows = if t.view > 0 { t.results.get(t.view - 1).map(|r| r.rows.len()).unwrap_or(0) } else { 0 };
                        ui.label(RichText::new(format!("Query time: {:.3}s{}", e.as_secs_f64(), if t.view > 0 { format!("   Records: {}", rows) } else { String::new() })).color(MUTED));
                    }
                });
            });
        });
        if t.view == 0 {
            let mut msg = String::new();
            for r in &t.results {
                msg.push_str(&r.statement);
                if r.has_rows {
                    msg.push_str(&format!("\n> OK, {} row(s) returned\n", r.rows.len()));
                } else {
                    msg.push_str(&format!("\n> Affected rows: {}\n", r.affected));
                }
                msg.push('\n');
            }
            if let Some((st, e)) = &t.error {
                msg.push_str(&format!("{}\n> {}\n", st, e));
            }
            if let Some(e) = t.elapsed {
                msg.push_str(&format!("> Time: {:.3}s\n", e.as_secs_f64()));
            }
            if msg.is_empty() {
                msg = "Write SQL above and press Run (Ctrl+R). Select part of the text to run just that part.".into();
            }
            egui::ScrollArea::both().auto_shrink(false).show(ui, |ui| {
                ui.add_space(4.0);
                let color = if t.error.is_some() { Color32::from_rgb(0xb0, 0x20, 0x20) } else { TEXT };
                ui.add(egui::Label::new(RichText::new(msg).monospace().color(color)).selectable(true));
            });
        } else if let Some(rs) = t.results.get(t.view - 1) {
            result_grid(ui, ("qgrid", t.id, t.view), &rs.columns, &rs.rows, &mut t.sel_cell);
        }
    });

    egui::CentralPanel::default().frame(egui::Frame::new().fill(Color32::WHITE)).show(ui, |ui| {
        egui::ScrollArea::both().id_salt(("qed", t.id)).auto_shrink(false).show(ui, |ui| {
            ui.horizontal_top(|ui| {
                let lines = t.sql.matches('\n').count() + 1;
                let nums: String = (1..=lines.max(30)).map(|n| n.to_string()).collect::<Vec<_>>().join("\n");
                egui::Frame::new().fill(HEADER_BG).inner_margin(egui::Margin { left: 6, right: 6, top: 2, bottom: 0 }).show(ui, |ui| {
                    ui.add(egui::Label::new(RichText::new(nums).font(FontId::monospace(13.5)).color(MUTED)).selectable(false));
                });
                let mut layouter = |ui: &Ui, text: &dyn egui::TextBuffer, wrap: f32| {
                    let mut job = highlight(text.as_str());
                    job.wrap.max_width = wrap;
                    ui.fonts_mut(|f| f.layout_job(job))
                };
                let out = egui::TextEdit::multiline(&mut t.sql)
                    .id_salt(("sql", t.id))
                    .font(FontId::monospace(13.5))
                    .code_editor()
                    .frame(egui::Frame::NONE)
                    .margin(egui::Margin { left: 6, right: 4, top: 2, bottom: 2 })
                    .desired_width(f32::INFINITY)
                    .desired_rows(30)
                    .lock_focus(true)
                    .layouter(&mut layouter)
                    .show(ui);
                if out.response.response.changed() {
                    t.dirty = true;
                }
                if let Some(cr) = out.cursor_range {
                    let r = cr.as_sorted_char_range();
                    t.selection = Some((r.start.0, r.end.0));
                }
            });
        });
    });
}

// ---------------------------------------------------------------- data tab

pub struct RowState {
    pub orig: Option<Row>,
    pub cur: Row,
    pub deleted: bool,
}

impl RowState {
    pub fn changed(&self) -> bool {
        self.deleted || self.orig.as_ref() != Some(&self.cur)
    }
}

pub struct DataTab {
    pub id: u64,
    pub cid: u64,
    pub db: String,
    pub table: String,
    pub is_view: bool,
    pub kind: DbKind,
    pub fields: Vec<FieldDef>,
    pub columns: Vec<String>,
    pub rows: Vec<RowState>,
    pub sel: Option<(usize, usize)>,
    pub editing: Option<(usize, usize, String, bool)>,
    pub filter: String,
    pub filter_applied: String,
    pub show_filter: bool,
    pub order: Option<(String, bool)>,
    pub limit: usize,
    pub offset: usize,
    pub total: Option<u64>,
    pub loading: bool,
    pub saving: bool,
    pub last_sql: String,
    pub error: Option<String>,
    pub elapsed: Option<Duration>,
    pub scroll_to: Option<usize>,
}

impl DataTab {
    pub fn reload_req(&mut self) -> Req {
        self.loading = true;
        Req::TableData { tab: self.id, db: self.db.clone(), table: self.table.clone(), filter: self.filter_applied.clone(), order: self.order.clone(), limit: self.limit, offset: self.offset }
    }
    pub fn dirty(&self) -> bool {
        self.rows.iter().any(|r| r.changed())
    }

    fn where_for(&self, row: &Row) -> String {
        let pg = self.kind.is_pg();
        let keys: Vec<usize> = self.fields.iter().enumerate().filter(|(_, f)| f.key).filter_map(|(_, f)| self.columns.iter().position(|c| c == &f.name)).collect();
        let idx: Vec<usize> = if keys.is_empty() { (0..self.columns.len()).collect() } else { keys };
        idx.iter()
            .map(|&i| match &row[i] {
                None => format!("{} IS NULL", sqlgen::quote_ident(pg, &self.columns[i])),
                v => format!("{} = {}", sqlgen::quote_ident(pg, &self.columns[i]), sqlgen::literal(pg, v)),
            })
            .collect::<Vec<_>>()
            .join(" AND ")
    }

    pub fn change_statements(&self) -> Vec<String> {
        let pg = self.kind.is_pg();
        let t = sqlgen::quote_ident(pg, &self.table);
        let no_key = !self.fields.iter().any(|f| f.key);
        let lim = if no_key && !pg { " LIMIT 1" } else { "" };
        let mut out = vec![];
        for r in &self.rows {
            match (&r.orig, r.deleted) {
                (Some(o), true) => out.push(format!("DELETE FROM {} WHERE {}{}", t, self.where_for(o), lim)),
                (None, true) => {}
                (Some(o), false) if o != &r.cur => {
                    let sets: Vec<String> = (0..self.columns.len()).filter(|&i| o[i] != r.cur[i]).map(|i| format!("{} = {}", sqlgen::quote_ident(pg, &self.columns[i]), sqlgen::literal(pg, &r.cur[i]))).collect();
                    out.push(format!("UPDATE {} SET {} WHERE {}{}", t, sets.join(", "), self.where_for(o), lim));
                }
                (None, false) => {
                    let idx: Vec<usize> = (0..self.columns.len()).filter(|&i| r.cur[i].is_some()).collect();
                    if idx.is_empty() {
                        out.push(if pg { format!("INSERT INTO {} DEFAULT VALUES", t) } else { format!("INSERT INTO {} () VALUES ()", t) });
                    } else {
                        out.push(format!(
                            "INSERT INTO {} ({}) VALUES ({})",
                            t,
                            idx.iter().map(|&i| sqlgen::quote_ident(pg, &self.columns[i])).collect::<Vec<_>>().join(", "),
                            idx.iter().map(|&i| sqlgen::literal(pg, &r.cur[i])).collect::<Vec<_>>().join(", ")
                        ));
                    }
                }
                _ => {}
            }
        }
        out
    }

    fn add_row(&mut self) {
        let n = self.columns.len();
        self.rows.push(RowState { orig: None, cur: vec![None; n], deleted: false });
        let r = self.rows.len() - 1;
        self.sel = Some((r, 0));
        self.scroll_to = Some(r);
    }
}

pub fn data_view(ui: &mut Ui, t: &mut DataTab, acts: &mut Vec<Act>) {
    let editable = !t.is_view;
    let save_shortcut = ui.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::S));
    // top toolbar
    toolbar_frame().show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 2.0;
            if tool_btn(ui, Some(Icon::Filter), if t.show_filter { "Hide Filter" } else { "Filter" }, true).clicked() {
                t.show_filter = !t.show_filter;
            }
            if tool_btn(ui, None, "Clear Sort", t.order.is_some()).clicked() {
                t.order = None;
                acts.push(Act::Send(t.cid, t.reload_req()));
            }
            sep(ui);
            if tool_btn(ui, Some(Icon::Import), "Import", editable).clicked() {
                acts.push(Act::Import(t.cid, t.db.clone(), Some(t.table.clone())));
            }
            if tool_btn(ui, Some(Icon::Export), "Export", true).clicked() {
                acts.push(Act::Export(t.cid, t.db.clone(), t.table.clone()));
            }
            sep(ui);
            if tool_btn(ui, Some(Icon::Design), "Design Table", editable).clicked() {
                acts.push(Act::DesignTable(t.cid, t.db.clone(), Some(t.table.clone())));
            }
            if t.loading || t.saving {
                ui.add_space(8.0);
                ui.spinner();
            }
        });
        if t.show_filter {
            ui.horizontal(|ui| {
                ui.label(RichText::new("WHERE").strong().color(ACCENT));
                let r = ui.add(egui::TextEdit::singleline(&mut t.filter).hint_text("e.g. country = 'Japan' AND id > 10").desired_width(ui.available_width() - 150.0).font(FontId::monospace(13.0)));
                let enter = r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                if ui.button("Apply").clicked() || enter {
                    t.filter_applied = t.filter.clone();
                    t.offset = 0;
                    acts.push(Act::Send(t.cid, t.reload_req()));
                }
                if ui.button("Clear").clicked() {
                    t.filter.clear();
                    t.filter_applied.clear();
                    t.offset = 0;
                    acts.push(Act::Send(t.cid, t.reload_req()));
                }
            });
        }
    });

    // bottom bar
    egui::Panel::bottom(("dbar", t.id)).exact_size(30.0).frame(toolbar_frame().stroke(Stroke::new(1.0, BORDER))).show(ui, |ui| {
        ui.horizontal_centered(|ui| {
            ui.spacing_mut().item_spacing.x = 1.0;
            if tool_btn(ui, Some(Icon::Add), "", editable).on_hover_text("Add record").clicked() {
                t.add_row();
            }
            if tool_btn(ui, Some(Icon::Minus), "", editable && t.sel.is_some()).on_hover_text("Delete record").clicked() {
                if let Some((r, _)) = t.sel {
                    if t.rows[r].orig.is_none() {
                        t.rows.remove(r);
                        t.sel = None;
                    } else {
                        t.rows[r].deleted = !t.rows[r].deleted;
                    }
                }
            }
            let dirty = t.dirty();
            if tool_btn(ui, Some(Icon::Check), "", dirty && !t.saving).on_hover_text("Apply changes (Ctrl+S)").clicked() || (save_shortcut && dirty) {
                commit_edit(t);
                let stmts = t.change_statements();
                if !stmts.is_empty() {
                    t.saving = true;
                    acts.push(Act::Send(t.cid, Req::Exec { tab: t.id, db: t.db.clone(), stmts, purpose: Purpose::SaveData }));
                }
            }
            if tool_btn(ui, Some(Icon::Discard), "", dirty).on_hover_text("Discard changes").clicked() {
                t.editing = None;
                t.rows.retain(|r| r.orig.is_some());
                for r in &mut t.rows {
                    r.deleted = false;
                    r.cur = r.orig.clone().unwrap();
                }
            }
            if tool_btn(ui, Some(Icon::Refresh), "", !t.loading).on_hover_text("Refresh (F5)").clicked() {
                acts.push(Act::Send(t.cid, t.reload_req()));
            }
            ui.add_space(10.0);
            ui.add(egui::Label::new(RichText::new(&t.last_sql).monospace().size(11.5).color(MUTED)).truncate());
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let total = t.total.unwrap_or(t.rows.len() as u64);
                let pages = ((total as usize).max(1) + t.limit - 1) / t.limit;
                let page = t.offset / t.limit + 1;
                if ui.add_enabled(page < pages, egui::Button::new("»").small()).clicked() {
                    t.offset = (pages - 1) * t.limit;
                    acts.push(Act::Send(t.cid, t.reload_req()));
                }
                if ui.add_enabled(page < pages, egui::Button::new("›").small()).clicked() {
                    t.offset += t.limit;
                    acts.push(Act::Send(t.cid, t.reload_req()));
                }
                ui.label(format!(" {} / {} ", page, pages));
                if ui.add_enabled(page > 1, egui::Button::new("‹").small()).clicked() {
                    t.offset = t.offset.saturating_sub(t.limit);
                    acts.push(Act::Send(t.cid, t.reload_req()));
                }
                if ui.add_enabled(page > 1, egui::Button::new("«").small()).clicked() {
                    t.offset = 0;
                    acts.push(Act::Send(t.cid, t.reload_req()));
                }
                ui.add_space(10.0);
                let rec = match t.sel {
                    Some((r, _)) => format!("Record {} of {}", t.offset + r + 1, total),
                    None => format!("{} record(s)", total),
                };
                ui.label(RichText::new(rec).color(MUTED));
            });
        });
    });

    egui::CentralPanel::default().frame(egui::Frame::new().fill(Color32::WHITE)).show(ui, |ui| {
        if let Some(e) = &t.error {
            ui.add_space(10.0);
            ui.colored_label(Color32::from_rgb(0xb0, 0x20, 0x20), format!("  {}", e));
            return;
        }
        if t.columns.is_empty() {
            ui.add_space(10.0);
            if t.loading {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label("Loading data…");
                });
            }
            return;
        }
        data_grid(ui, t, editable, acts);
    });
}

fn commit_edit(t: &mut DataTab) {
    if let Some((r, c, text, _)) = t.editing.take() {
        if let Some(row) = t.rows.get_mut(r) {
            if row.cur[c].as_deref() != Some(text.as_str()) {
                if !(row.cur[c].is_none() && text.is_empty()) {
                    row.cur[c] = Some(text);
                }
            }
        }
    }
}

fn data_grid(ui: &mut Ui, t: &mut DataTab, editable: bool, acts: &mut Vec<Act>) {
    // keyboard navigation
    let no_focus = ui.ctx().memory(|m| m.focused().is_none());
    if t.editing.is_none() && no_focus {
        if let Some((r, c)) = t.sel {
            let (mut nr, mut nc) = (r, c);
            ui.input(|i| {
                if i.key_pressed(egui::Key::ArrowDown) {
                    nr = (r + 1).min(t.rows.len().saturating_sub(1));
                }
                if i.key_pressed(egui::Key::ArrowUp) {
                    nr = r.saturating_sub(1);
                }
                if i.key_pressed(egui::Key::ArrowRight) || i.key_pressed(egui::Key::Tab) {
                    nc = (c + 1).min(t.columns.len().saturating_sub(1));
                }
                if i.key_pressed(egui::Key::ArrowLeft) {
                    nc = c.saturating_sub(1);
                }
            });
            if (nr, nc) != (r, c) {
                t.sel = Some((nr, nc));
                t.scroll_to = Some(nr);
            }
            if editable && ui.input(|i| i.key_pressed(egui::Key::Enter) || i.key_pressed(egui::Key::F2)) && r < t.rows.len() {
                t.editing = Some((r, c, t.rows[r].cur[c].clone().unwrap_or_default(), true));
            }
            if ui.input(|i| i.modifiers.command && i.key_pressed(egui::Key::C)) {
                if let Some(v) = t.rows.get(r).and_then(|row| row.cur.get(c)) {
                    ui.ctx().copy_text(v.clone().unwrap_or_default());
                }
            }
        }
    }
    let keys: Vec<bool> = t.columns.iter().map(|c| t.fields.iter().any(|f| &f.name == c && f.key)).collect();
    let preview: Vec<Row> = t.rows.iter().take(50).map(|r| r.cur.clone()).collect();
    let mut header_click: Option<usize> = None;
    let mut start_edit: Option<(usize, usize)> = None;
    let mut cell_menu: Option<(usize, usize, &'static str)> = None;
    let n_cols = t.columns.len();
    let n_rows = t.rows.len();
    ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
    let mut tb = TableBuilder::new(ui)
        .id_salt(("dgrid", t.id))
        .striped(true)
        .sense(Sense::click())
        .resizable(true)
        .auto_shrink(false)
        .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
        .column(Column::exact(36.0));
    for (i, c) in t.columns.iter().enumerate() {
        tb = tb.column(Column::initial(col_width(c, &preview, i) + if keys[i] { 16.0 } else { 0.0 }).at_least(40.0).clip(true));
    }
    if let Some(r) = t.scroll_to.take() {
        tb = tb.scroll_to_row(r, None);
    }
    let order = t.order.clone();
    tb.header(24.0, |mut h| {
        h.col(|ui| {
            ui.painter().rect_filled(ui.max_rect(), 0.0, HEADER_BG);
            cell_lines(ui, ui.max_rect());
        });
        for (i, c) in t.columns.iter().enumerate() {
            let (_, resp) = h.col(|ui| {
                let rect = ui.max_rect();
                ui.painter().rect_filled(rect, 0.0, HEADER_BG);
                cell_lines(ui, rect);
                ui.add_space(5.0);
                if keys[i] {
                    let (r, _) = ui.allocate_exact_size(vec2(14.0, 14.0), Sense::hover());
                    icons::paint(ui.painter(), r, Icon::Key);
                    ui.add_space(2.0);
                }
                ui.add(egui::Label::new(RichText::new(c).strong()).truncate().selectable(false));
                if let Some((oc, desc)) = &order {
                    if oc == c {
                        arrow(ui.painter(), pos2(rect.right() - 10.0, rect.center().y), 4.0, if *desc { 1 } else { 3 }, ACCENT);
                    }
                }
            });
            if resp.clicked() {
                header_click = Some(i);
            }
        }
    })
    .body(|body| {
        body.rows(22.0, n_rows, |mut row| {
            let r = row.index();
            let rs = &t.rows[r];
            let bg = if rs.deleted {
                Some(DELETED_BG)
            } else if rs.orig.is_none() {
                Some(NEW_BG)
            } else {
                None
            };
            row.col(|ui| {
                let rect = ui.max_rect();
                ui.painter().rect_filled(rect, 0.0, HEADER_BG);
                cell_lines(ui, rect);
                let mr = Rect::from_center_size(pos2(rect.left() + 13.0, rect.center().y), vec2(12.0, 12.0));
                if rs.deleted {
                    icons::paint(ui.painter(), mr, Icon::Delete);
                } else if rs.orig.is_none() {
                    icons::paint(ui.painter(), mr, Icon::Add);
                } else if rs.changed() {
                    ui.painter().circle_filled(mr.center(), 3.5, Color32::from_rgb(0xe8, 0x8f, 0x2a));
                } else if matches!(t.sel, Some((sr, _)) if sr == r) {
                    arrow(ui.painter(), mr.center(), 4.0, 0, ACCENT);
                }
            });
            for c in 0..n_cols {
                let editing_here = matches!(&t.editing, Some((er, ec, _, _)) if *er == r && *ec == c);
                let (_, resp) = row.col(|ui| {
                    let rect = ui.max_rect();
                    let rs = &t.rows[r];
                    let cell_changed = rs.orig.as_ref().map(|o| o[c] != rs.cur[c]).unwrap_or(false);
                    if let Some(bg) = bg {
                        ui.painter().rect_filled(rect, 0.0, bg);
                    } else if cell_changed {
                        ui.painter().rect_filled(rect, 0.0, EDITED_BG);
                    }
                    if t.sel == Some((r, c)) && !editing_here {
                        ui.painter().rect_filled(rect, 0.0, SEL_BG);
                    }
                    cell_lines(ui, rect);
                    if editing_here {
                        let Some((_, _, text, first)) = &mut t.editing else { return };
                        let te = ui.add(egui::TextEdit::singleline(text).desired_width(rect.width() - 2.0).margin(egui::Margin::symmetric(4, 1)));
                        if *first {
                            te.request_focus();
                            *first = false;
                        } else if te.lost_focus() {
                            if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                                t.editing = None;
                            } else {
                                commit_edit(t);
                            }
                        }
                    } else {
                        let color = if t.rows[r].deleted { MUTED } else { TEXT };
                        cell_text(ui, &t.rows[r].cur[c], color);
                    }
                });
                if !editing_here {
                    if resp.clicked() || resp.secondary_clicked() {
                        if t.editing.is_some() {
                            commit_edit(t);
                        }
                        t.sel = Some((r, c));
                    }
                    if resp.double_clicked() && editable {
                        start_edit = Some((r, c));
                    }
                    resp.context_menu(|ui| {
                        if editable {
                            if ui.button("Edit Cell").clicked() {
                                cell_menu = Some((r, c, "edit"));
                                ui.close();
                            }
                            if ui.button("Set to NULL").clicked() {
                                cell_menu = Some((r, c, "null"));
                                ui.close();
                            }
                            if ui.button("Set to Empty String").clicked() {
                                cell_menu = Some((r, c, "empty"));
                                ui.close();
                            }
                            ui.separator();
                        }
                        if ui.button("Copy").clicked() {
                            cell_menu = Some((r, c, "copy"));
                            ui.close();
                        }
                        if ui.button("Copy as INSERT Statement").clicked() {
                            cell_menu = Some((r, c, "copy_insert"));
                            ui.close();
                        }
                        if ui.button("Filter by This Value").clicked() {
                            cell_menu = Some((r, c, "filter"));
                            ui.close();
                        }
                        if editable {
                            ui.separator();
                            if ui.button("Add Record").clicked() {
                                cell_menu = Some((r, c, "add"));
                                ui.close();
                            }
                            if ui.button("Delete Record").clicked() {
                                cell_menu = Some((r, c, "delete"));
                                ui.close();
                            }
                        }
                    });
                }
            }
        });
    });
    if let Some(i) = header_click {
        let c = t.columns[i].clone();
        t.order = match &t.order {
            Some((oc, false)) if *oc == c => Some((c, true)),
            Some((oc, true)) if *oc == c => None,
            _ => Some((c, false)),
        };
        acts.push(Act::Send(t.cid, t.reload_req()));
    }
    if let Some((r, c)) = start_edit {
        t.sel = Some((r, c));
        t.editing = Some((r, c, t.rows[r].cur[c].clone().unwrap_or_default(), true));
    }
    if let Some((r, c, what)) = cell_menu {
        let pg = t.kind.is_pg();
        match what {
            "edit" => t.editing = Some((r, c, t.rows[r].cur[c].clone().unwrap_or_default(), true)),
            "null" => t.rows[r].cur[c] = None,
            "empty" => t.rows[r].cur[c] = Some(String::new()),
            "copy" => ui.ctx().copy_text(t.rows[r].cur[c].clone().unwrap_or_default()),
            "copy_insert" => {
                let sql = format!(
                    "INSERT INTO {} ({}) VALUES ({});",
                    sqlgen::quote_ident(pg, &t.table),
                    t.columns.iter().map(|c| sqlgen::quote_ident(pg, c)).collect::<Vec<_>>().join(", "),
                    t.rows[r].cur.iter().map(|v| sqlgen::literal(pg, v)).collect::<Vec<_>>().join(", ")
                );
                ui.ctx().copy_text(sql);
            }
            "filter" => {
                let col = sqlgen::quote_ident(pg, &t.columns[c]);
                t.filter = match &t.rows[r].cur[c] {
                    None => format!("{} IS NULL", col),
                    v => format!("{} = {}", col, sqlgen::literal(pg, v)),
                };
                t.filter_applied = t.filter.clone();
                t.show_filter = true;
                t.offset = 0;
                acts.push(Act::Send(t.cid, t.reload_req()));
            }
            "add" => t.add_row(),
            "delete" => {
                if t.rows[r].orig.is_none() {
                    t.rows.remove(r);
                    t.sel = None;
                } else {
                    t.rows[r].deleted = true;
                }
            }
            _ => {}
        }
    }
}

// ---------------------------------------------------------------- design tab

pub struct DesignTab {
    pub id: u64,
    pub cid: u64,
    pub db: String,
    pub kind: DbKind,
    pub table: Option<String>,
    pub name: String,
    pub fields: Vec<FieldDef>,
    pub orig: Vec<FieldDef>,
    pub comment: String,
    pub orig_comment: String,
    pub sub: usize,
    pub sel: Option<usize>,
    pub loading: bool,
    pub saving: bool,
    pub error: Option<String>,
}

impl DesignTab {
    pub fn statements(&self) -> Vec<String> {
        let pg = self.kind.is_pg();
        match &self.table {
            None => {
                if self.name.trim().is_empty() || self.fields.iter().all(|f| f.name.trim().is_empty()) {
                    vec![]
                } else {
                    sqlgen::create_table(pg, self.name.trim(), &self.fields, &self.comment)
                }
            }
            Some(t) => sqlgen::alter_table(pg, t, &self.orig, &self.fields, &self.orig_comment, &self.comment, self.name.trim()),
        }
    }
}

const MY_TYPES: &[&str] = &["int", "bigint", "smallint", "tinyint", "decimal", "float", "double", "varchar", "char", "text", "mediumtext", "longtext", "date", "datetime", "timestamp", "time", "year", "json", "blob", "enum", "bit"];
const PG_TYPES: &[&str] = &["integer", "bigint", "smallint", "numeric", "real", "double precision", "varchar", "char", "text", "boolean", "date", "timestamp", "timestamptz", "time", "interval", "json", "jsonb", "uuid", "bytea", "inet"];

pub fn design_view(ui: &mut Ui, t: &mut DesignTab, acts: &mut Vec<Act>) {
    let save_shortcut = ui.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::S));
    toolbar_frame().show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 2.0;
            let stmts = t.statements();
            if tool_btn(ui, Some(Icon::Save), "Save", !stmts.is_empty() && !t.saving).on_hover_text("Ctrl+S").clicked() || (save_shortcut && !stmts.is_empty()) {
                if t.name.trim().is_empty() {
                    acts.push(Act::Alert("Save table".into(), "Please enter a table name first.".into()));
                } else {
                    t.saving = true;
                    t.error = None;
                    acts.push(Act::Send(t.cid, Req::Exec { tab: t.id, db: t.db.clone(), stmts, purpose: Purpose::SaveDesign { new_name: t.name.trim().to_string() } }));
                }
            }
            sep(ui);
            if tool_btn(ui, Some(Icon::Add), "Add Field", true).clicked() {
                t.fields.push(FieldDef { ty: if t.kind.is_pg() { "varchar".into() } else { "varchar".into() }, length: "255".into(), ..Default::default() });
                t.sel = Some(t.fields.len() - 1);
            }
            if tool_btn(ui, None, "Insert Field", true).clicked() {
                let at = t.sel.unwrap_or(0).min(t.fields.len());
                t.fields.insert(at, FieldDef { ty: "varchar".into(), length: "255".into(), ..Default::default() });
                t.sel = Some(at);
            }
            if tool_btn(ui, Some(Icon::Delete), "Delete Field", t.sel.is_some()).clicked() {
                if let Some(i) = t.sel {
                    if i < t.fields.len() {
                        t.fields.remove(i);
                    }
                    t.sel = if t.fields.is_empty() { None } else { Some(i.min(t.fields.len() - 1)) };
                }
            }
            if tool_btn(ui, Some(Icon::Key), "Primary Key", t.sel.is_some()).clicked() {
                if let Some(i) = t.sel {
                    let f = &mut t.fields[i];
                    f.key = !f.key;
                    if f.key {
                        f.not_null = true;
                    }
                }
            }
            sep(ui);
            if tool_btn(ui, None, "Move Up", matches!(t.sel, Some(i) if i > 0)).clicked() {
                let i = t.sel.unwrap();
                t.fields.swap(i, i - 1);
                t.sel = Some(i - 1);
            }
            if tool_btn(ui, None, "Move Down", matches!(t.sel, Some(i) if i + 1 < t.fields.len())).clicked() {
                let i = t.sel.unwrap();
                t.fields.swap(i, i + 1);
                t.sel = Some(i + 1);
            }
            sep(ui);
            ui.label("Table name:");
            ui.add(egui::TextEdit::singleline(&mut t.name).desired_width(180.0));
            if t.loading || t.saving {
                ui.spinner();
            }
        });
    });
    egui::Frame::new().fill(HEADER_BG).inner_margin(egui::Margin::symmetric(6, 2)).show(ui, |ui| {
        ui.horizontal(|ui| {
            for (i, l) in ["Fields", "Comment", "SQL Preview"].iter().enumerate() {
                if ui.selectable_label(t.sub == i, *l).clicked() {
                    t.sub = i;
                }
            }
        });
    });
    if let Some(e) = &t.error {
        ui.colored_label(Color32::from_rgb(0xb0, 0x20, 0x20), format!("  {}", e));
    }
    match t.sub {
        1 => {
            ui.add_space(6.0);
            ui.label("  Table comment:");
            ui.add_sized([ui.available_width() - 12.0, 120.0], egui::TextEdit::multiline(&mut t.comment));
        }
        2 => {
            let mut s = t.statements().iter().map(|s| format!("{};", s)).collect::<Vec<_>>().join("\n\n");
            if s.is_empty() {
                s = "-- No changes".into();
            }
            egui::ScrollArea::both().auto_shrink(false).show(ui, |ui| {
                ui.add(egui::TextEdit::multiline(&mut s).font(FontId::monospace(13.0)).desired_width(f32::INFINITY).interactive(true).frame(egui::Frame::NONE).layouter(&mut |ui: &Ui, text: &dyn egui::TextBuffer, wrap: f32| {
                    let mut job = highlight(text.as_str());
                    job.wrap.max_width = wrap;
                    ui.fonts_mut(|f| f.layout_job(job))
                }));
            });
        }
        _ => {
            egui::Panel::bottom(("dprops", t.id)).resizable(true).default_size(120.0).frame(egui::Frame::new().fill(TOOLBAR_BG).inner_margin(8).stroke(Stroke::new(1.0, BORDER))).show(ui, |ui| {
                if let Some(i) = t.sel.filter(|i| *i < t.fields.len()) {
                    let f = &mut t.fields[i];
                    egui::Grid::new(("fprops", t.id)).num_columns(2).spacing([12.0, 6.0]).show(ui, |ui| {
                        ui.label("Default:");
                        ui.add(egui::TextEdit::singleline(&mut f.default).hint_text("(none)").desired_width(260.0));
                        ui.end_row();
                        ui.label("Auto Increment:");
                        ui.checkbox(&mut f.auto_inc, "");
                        ui.end_row();
                        ui.label("Comment:");
                        ui.add(egui::TextEdit::singleline(&mut f.comment).desired_width(260.0));
                        ui.end_row();
                    });
                } else {
                    ui.label(RichText::new("Select a field to see its properties.").color(MUTED));
                }
            });
            egui::CentralPanel::default().frame(egui::Frame::new().fill(Color32::WHITE)).show(ui, |ui| {
                design_grid(ui, t);
            });
        }
    }
}

fn design_grid(ui: &mut Ui, t: &mut DesignTab) {
    let types = if t.kind.is_pg() { PG_TYPES } else { MY_TYPES };
    let n = t.fields.len();
    let mut select: Option<usize> = None;
    ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
    TableBuilder::new(ui)
        .id_salt(("dsgrid", t.id))
        .striped(true)
        .resizable(true)
        .auto_shrink(false)
        .sense(Sense::click())
        .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
        .column(Column::exact(28.0))
        .column(Column::initial(180.0).at_least(80.0).clip(true))
        .column(Column::initial(170.0).at_least(80.0).clip(true))
        .column(Column::initial(80.0).clip(true))
        .column(Column::initial(80.0).clip(true))
        .column(Column::initial(76.0).clip(true))
        .column(Column::initial(56.0).clip(true))
        .column(Column::remainder().at_least(120.0))
        .header(24.0, |mut h| {
            for l in ["", "Name", "Type", "Length", "Decimals", "Not Null", "Key", "Comment"] {
                h.col(|ui| {
                    ui.painter().rect_filled(ui.max_rect(), 0.0, HEADER_BG);
                    cell_lines(ui, ui.max_rect());
                    ui.add_space(5.0);
                    ui.label(RichText::new(l).strong());
                });
            }
        })
        .body(|body| {
            body.rows(26.0, n, |mut row| {
                let i = row.index();
                let selected = t.sel == Some(i);
                row.set_selected(selected);
                let (_, r0) = row.col(|ui| {
                    let rect = ui.max_rect();
                    ui.painter().rect_filled(rect, 0.0, HEADER_BG);
                    cell_lines(ui, rect);
                    if selected {
                        arrow(ui.painter(), pos2(rect.left() + 13.0, rect.center().y), 4.0, 0, ACCENT);
                    }
                });
                if r0.clicked() {
                    select = Some(i);
                }
                let f = &mut t.fields[i];
                let mut touched = false;
                row.col(|ui| {
                    cell_lines(ui, ui.max_rect());
                    ui.add_space(2.0);
                    touched |= ui.add(egui::TextEdit::singleline(&mut f.name).desired_width(ui.available_width() - 6.0).id_salt(("fname", i))).gained_focus();
                });
                row.col(|ui| {
                    cell_lines(ui, ui.max_rect());
                    ui.add_space(2.0);
                    touched |= ui.add(egui::TextEdit::singleline(&mut f.ty).desired_width(ui.available_width() - 40.0).id_salt(("ftype", i))).gained_focus();
                    ui.menu_button("…", |ui| {
                        egui::ScrollArea::vertical().max_height(300.0).show(ui, |ui| {
                            for ty in types {
                                if ui.button(*ty).clicked() {
                                    f.ty = ty.to_string();
                                    if !["varchar", "char", "decimal", "numeric"].contains(ty) {
                                        f.length.clear();
                                        f.decimals.clear();
                                    } else if f.length.is_empty() {
                                        f.length = if ty.contains("char") { "255".into() } else { "10".into() };
                                        if !ty.contains("char") {
                                            f.decimals = "2".into();
                                        }
                                    }
                                    ui.close();
                                }
                            }
                        });
                    });
                });
                row.col(|ui| {
                    cell_lines(ui, ui.max_rect());
                    ui.add_space(2.0);
                    touched |= ui.add(egui::TextEdit::singleline(&mut f.length).desired_width(ui.available_width() - 6.0).id_salt(("flen", i))).gained_focus();
                });
                row.col(|ui| {
                    cell_lines(ui, ui.max_rect());
                    ui.add_space(2.0);
                    touched |= ui.add(egui::TextEdit::singleline(&mut f.decimals).desired_width(ui.available_width() - 6.0).id_salt(("fdec", i))).gained_focus();
                });
                row.col(|ui| {
                    cell_lines(ui, ui.max_rect());
                    ui.add_space(24.0);
                    touched |= ui.checkbox(&mut f.not_null, "").clicked();
                });
                let (_, rk) = row.col(|ui| {
                    let rect = ui.max_rect();
                    cell_lines(ui, rect);
                    if f.key {
                        icons::paint(ui.painter(), Rect::from_center_size(rect.center(), vec2(16.0, 16.0)), Icon::Key);
                    }
                });
                if rk.clicked() {
                    f.key = !f.key;
                    if f.key {
                        f.not_null = true;
                    }
                    touched = true;
                }
                row.col(|ui| {
                    cell_lines(ui, ui.max_rect());
                    ui.add_space(2.0);
                    touched |= ui.add(egui::TextEdit::singleline(&mut f.comment).desired_width(ui.available_width() - 6.0).id_salt(("fcom", i))).gained_focus();
                });
                if touched {
                    select = Some(i);
                }
            });
        });
    if let Some(i) = select {
        t.sel = Some(i);
    }
}

// ---------------------------------------------------------------- dialogs data

pub struct ExportState {
    pub cid: u64,
    pub db: String,
    pub table: String,
    pub format: ExportFormat,
    pub path: String,
    pub header: bool,
}
