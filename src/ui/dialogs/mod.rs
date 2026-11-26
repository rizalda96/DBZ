//! Pop-up windows: alerts, confirmations, wizards and the connection editor. They edit
//! their own state and push `Act`s; the app does the work.

mod conn_auth;
mod connection;
mod simple;
mod wizards;

use eframe::egui;

use crate::app::{Act, Dialog, DialogState, Workspace};
use crate::ui::widgets::Choice;

/// Draws whichever dialog is open. The alert wins over everything else.
pub fn show(ctx: &egui::Context, st: &mut DialogState, ws: &Workspace, acts: &mut Vec<Act>) {
    if let Some(task) = &st.task {
        simple::progress(ctx, task);
        return;
    }
    if let Some((title, msg)) = &st.alert {
        if !simple::alert(ctx, title, msg) {
            st.alert = None;
        }
        return;
    }
    if let Some(d) = &mut st.conn {
        connection::show(ctx, d, acts);
    }
    let Some(dialog) = &mut st.dialog else { return };
    let choice = draw(ctx, dialog, ws, acts);
    if choice.keep() {
        return;
    }
    if let (Choice::Ok, Some(Dialog::Confirm { action, .. })) = (choice, st.dialog.take()) {
        acts.push(Act::Confirmed(action));
    }
    st.dialog = None;
}

fn draw(ctx: &egui::Context, dialog: &mut Dialog, ws: &Workspace, acts: &mut Vec<Act>) -> Choice {
    match dialog {
        Dialog::Confirm { title, msg, .. } => simple::confirm(ctx, title, msg),
        Dialog::NewDb { cid, name, charset } => simple::new_db(ctx, (*cid, ws.kind_of(*cid)), name, charset, acts),
        Dialog::SaveQuery { tab, name } => simple::save_query(ctx, *tab, name, acts),
        Dialog::NameMatView { tab, name } => simple::name_matview(ctx, *tab, name, acts),
        Dialog::Rename { cid, db, table, new } => simple::rename(ctx, (*cid, ws.dialect_of(*cid)), (db, table), new, acts),
        Dialog::About => simple::about(ctx),
        Dialog::Export { cid, db, opts } => wizards::export(ctx, (*cid, db), opts, acts),
        Dialog::Import { cid, db, opts } => {
            let tables: Vec<String> = ws.conn(*cid).and_then(|c| c.db(db)).map(|d| d.tables.iter().filter(|t| !t.is_view).map(|t| t.name.clone()).collect()).unwrap_or_default();
            wizards::import(ctx, (*cid, db), opts, &tables, acts)
        }
        Dialog::RunFile { cid, db, path, cont } => wizards::run_file(ctx, (*cid, db), path, cont, acts),
    }
}
