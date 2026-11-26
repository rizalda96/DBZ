//! Export, Import, Execute SQL File and Import Connections wizards.

use std::path::Path;

use eframe::egui::{self, RichText, Ui};

use crate::ui::widgets::{buttons, file_row, win, Choice};
use crate::app::Act;
use crate::shared::sanitize;
use crate::transfer::{ConnSource, ExportFormat, ExportOptions, ImportOptions};
use crate::worker::Req;

pub(super) fn export(ctx: &egui::Context, (cid, db): (u64, &str), opts: &mut ExportOptions, acts: &mut Vec<Act>) -> Choice {
    let mut choice = Choice::None;
    win("Export Wizard").show(ctx, |ui| {
        ui.label(RichText::new(format!("Export the data of \"{}\" to a file.", opts.table)).strong());
        ui.add_space(6.0);
        ui.label("Format:");
        for f in ExportFormat::ALL {
            if ui.radio_value(&mut opts.format, f, f.label()).changed() {
                opts.path = Path::new(&opts.path).with_extension(f.ext()).to_string_lossy().to_string();
            }
        }
        ui.add_space(6.0);
        ui.checkbox(&mut opts.header, "Include column titles");
        ui.add_space(6.0);
        ui.label("Save to:");
        let (label, ext) = (opts.format.label(), opts.format.ext());
        let fname = Path::new(&opts.path).file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
        file_row(ui, &mut opts.path, "", || rfd::FileDialog::new().add_filter(label, &[ext]).set_file_name(fname).save_file());
        choice = buttons(ui, "Export", !opts.path.trim().is_empty());
    });
    if choice == Choice::Ok {
        acts.push(Act::Status(format!("Exporting {}…", opts.table)));
        acts.push(Act::Send(cid, Req::Export { db: db.to_string(), opts: opts.clone() }));
    }
    choice
}

pub(super) fn import(ctx: &egui::Context, (cid, db): (u64, &str), opts: &mut ImportOptions, tables: &[String], acts: &mut Vec<Act>) -> Choice {
    let mut choice = Choice::None;
    win("Import Wizard").show(ctx, |ui| {
        ui.label(RichText::new(format!("Import a CSV or text file into \"{}\".", db)).strong());
        ui.add_space(6.0);
        ui.label("Source file:");
        let before = opts.path.clone();
        file_row(ui, &mut opts.path, "Choose a file…", || rfd::FileDialog::new().add_filter("CSV / text", &["csv", "txt", "tsv"]).pick_file());
        if opts.path != before {
            guess_from_path(opts);
        }
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.label("Field separator:");
            ui.radio_value(&mut opts.delim, ',', "Comma");
            ui.radio_value(&mut opts.delim, ';', "Semicolon");
            ui.radio_value(&mut opts.delim, '\t', "Tab");
        });
        ui.checkbox(&mut opts.header, "First row holds the field names");
        ui.add_space(6.0);
        target_table(ui, opts, tables);
        choice = buttons(ui, "Import", !opts.path.trim().is_empty() && !opts.table.trim().is_empty());
    });
    if choice == Choice::Ok {
        acts.push(Act::Status(format!("Importing into {}…", opts.table)));
        let opts = ImportOptions { table: opts.table.trim().to_string(), ..opts.clone() };
        acts.push(Act::Send(cid, Req::Import { db: db.to_string(), opts }));
    }
    choice
}

/// A new table is named after the file; .tsv / .txt files are tab separated.
fn guess_from_path(opts: &mut ImportOptions) {
    let p = Path::new(&opts.path);
    if opts.create && opts.table.is_empty() {
        opts.table = p.file_stem().map(|s| sanitize(&s.to_string_lossy()).replace(' ', "_")).unwrap_or_default();
    }
    if p.extension().is_some_and(|e| e == "tsv" || e == "txt") {
        opts.delim = '\t';
    }
}

fn target_table(ui: &mut Ui, opts: &mut ImportOptions, tables: &[String]) {
    ui.horizontal(|ui| {
        ui.radio_value(&mut opts.create, false, "Into an existing table");
        ui.radio_value(&mut opts.create, true, "Into a new table");
    });
    if opts.create {
        ui.horizontal(|ui| {
            ui.label("New table name:");
            ui.add(egui::TextEdit::singleline(&mut opts.table).desired_width(220.0));
        });
        return;
    }
    let shown = if opts.table.is_empty() { "(choose)".to_string() } else { opts.table.clone() };
    egui::ComboBox::from_label("Target table").selected_text(shown).show_ui(ui, |ui| {
        for t in tables {
            ui.selectable_value(&mut opts.table, t.clone(), t);
        }
    });
}

pub(super) fn run_file(ctx: &egui::Context, (cid, db): (u64, &str), path: &mut String, cont: &mut bool, acts: &mut Vec<Act>) -> Choice {
    let mut choice = Choice::None;
    win("Execute SQL File").show(ctx, |ui| {
        ui.label(format!("Run every statement of a .sql file in \"{}\".", db));
        ui.add_space(6.0);
        file_row(ui, path, "Choose a file…", || rfd::FileDialog::new().add_filter("SQL file", &["sql", "txt"]).pick_file());
        ui.checkbox(cont, "Continue on error");
        choice = buttons(ui, "Start", !path.trim().is_empty());
    });
    if choice == Choice::Ok {
        acts.push(Act::Status("Running SQL file…".into()));
        acts.push(Act::Send(cid, Req::RunFile { db: db.to_string(), path: path.clone(), continue_on_error: *cont }));
    }
    choice
}

pub(super) fn import_conns(ctx: &egui::Context, source: &mut ConnSource, path: &mut String, acts: &mut Vec<Act>) -> Choice {
    let mut choice = Choice::None;
    win("Import Connections").show(ctx, |ui| {
        ui.set_max_width(380.0);
        ui.label(RichText::new("Add the saved connections of another tool, passwords included.").strong());
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            for s in ConnSource::ALL {
                if ui.radio_value(source, s, s.importer().label()).changed() {
                    path.clear();
                }
            }
        });
        let importer = source.importer();
        ui.add_space(6.0);
        ui.label(RichText::new(importer.hint()).weak());
        ui.add_space(4.0);
        let (name, exts) = importer.filter();
        file_row(ui, path, "(default location)", || rfd::FileDialog::new().add_filter(name, exts).add_filter("All files", &["*"]).pick_file());
        choice = buttons(ui, "Import", true);
    });
    if choice == Choice::Ok {
        acts.push(Act::ImportConns(*source, path.clone()));
    }
    choice
}
