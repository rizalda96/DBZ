//! Routes each `Act` to its handler. One line per action keeps this a table of
//! contents; the work lives in the `*_actions` modules.

use super::ai_images::image_from_file;
use super::{Act, Dbz, Dialog};
use crate::transfer::ConnSource;

impl Dbz {
    pub fn do_act(&mut self, act: Act) {
        match act {
            Act::Send(cid, req) => self.send(cid, req),
            Act::EnsureOpen(cid) => self.ensure_open(cid),
            Act::OpenConn(cid) => self.open_conn(cid),
            Act::CloseConn(cid) => self.close_conn(cid),
            Act::ToggleConn(cid) => self.toggle_conn(cid),
            Act::EditConn(cid) => self.edit_conn(cid),
            Act::NewConn(kind) => self.new_conn(kind),
            Act::DupConn(cid) => self.dup_conn(cid),
            Act::DeleteConn(cid) => self.delete_conn(cid),
            Act::RefreshConn(cid) => self.refresh_conn(cid),
            Act::TestConn(cfg) => self.test_conn(cfg),
            Act::SaveConn { editing, cfg } => self.save_conn(editing, cfg),
            Act::CloseConnDialog => self.close_conn_dialog(),
            Act::ShowImportConns => self.dialogs.dialog = Some(Dialog::ImportConns { source: ConnSource::Navicat, path: String::new() }),
            Act::ImportConns(source, path) => self.import_conns(source, &path),
            Act::LoadUsers(cid) => self.load_users(cid),
            Act::ToggleDb(cid, db) => self.toggle_db(cid, db),
            Act::OpenDb(cid, db) => self.open_db(cid, db),
            Act::CloseDb(cid, db) => self.close_db(cid, &db),
            Act::RefreshDb(cid, db) => self.refresh_db(cid, db),
            Act::NewDb(cid) => self.new_db(cid),
            Act::DropDb(cid, db) => self.drop_db(cid, db),
            Act::Select(sel) => self.ws.sel = sel,
            Act::SelectFolder(f) => self.select_folder(f),
            Act::ToggleFolder(f) => self.toggle_folder(f),
            Act::ToggleSchema(cid, db, schema) => self.toggle_schema(cid, &db, schema),
            Act::SetMode(m) => self.set_mode(m),
            Act::LoadDdl(obj, is_view) => self.load_ddl(obj, is_view),
            Act::OpenTable(obj, is_view) => self.open_table(obj, is_view),
            Act::DesignTable(cid, db, table) => self.design_table(cid, db, table),
            Act::DesignView(obj) => self.design_view(obj),
            Act::RunView(tab) => self.run_view(tab),
            Act::DropTables(cid, db, names, view) => self.ask_drop_tables(cid, db, names, view),
            Act::Truncate(obj) => self.ask_truncate(obj),
            Act::Rename(obj) => self.dialogs.dialog = Some(Dialog::Rename { cid: obj.cid, db: obj.db, new: obj.name.clone(), table: obj.name }),
            Act::DropRoutine(obj, kind) => self.ask_drop_routine(obj, kind),
            Act::DesignRoutine(obj) => self.design_routine(obj),
            Act::ExecRoutine(obj) => self.exec_routine(obj),
            Act::NewRoutine(cid, db) => {
                let sql = self.ws.dialect_of(cid).templates().new_function.to_string();
                self.new_query(cid, db, sql)
            }
            Act::NewQuery(cid, db, sql) => self.new_query(cid, db, sql),
            Act::QueryTable(obj) => {
                let sql = self.ws.dialect_of(obj.cid).sample_query(&obj.name);
                self.new_query(obj.cid, obj.db, sql)
            }
            Act::OpenSavedQuery(obj) => self.open_saved_query(obj),
            Act::DeleteQuery(obj) => self.ask_delete_query(obj),
            Act::SaveQuery(tab) => self.save_query(tab),
            Act::WriteQuery(tab, name) => self.write_query(tab, name),
            Act::NewMatView(cid, db) => self.new_matview(cid, db),
            Act::SaveMatView(tab) => self.save_matview(tab),
            Act::NameMatView(tab, name) => self.name_matview(tab, name),
            Act::OpenSqlFile => self.open_sql_file(),
            Act::Export(obj) => self.export_dialog(obj),
            Act::Import(cid, db, table) => self.import_dialog(cid, db, table),
            Act::Dump(cid, db, data) => self.dump(cid, db, data),
            Act::DumpArchive(cid, db, part) => self.dump_archive(cid, db, part),
            Act::RestoreArchive(cid, db) => self.ask_restore_archive(cid, db),
            Act::RunFile(cid, db) => self.dialogs.dialog = Some(Dialog::RunFile { cid, db, path: String::new(), cont: false }),
            Act::NewBackup(cid, db) => self.new_backup(cid, db),
            Act::RestoreBackup(obj) => self.ask_restore_backup(obj),
            Act::DeleteBackup(obj) => self.ask_delete_backup(obj),
            Act::Confirmed(action) => self.run_confirm(action),
            Act::CloseTab(i) => self.ask_close_tab(i),
            Act::Activate(i) => self.ws.active = i.min(self.ws.tabs.len()),
            Act::Alert(t, m) => self.alert(t, m),
            Act::Status(s) => self.status = s,
            Act::SetTheme(t) => self.set_theme(t),
            Act::ShowAbout => self.dialogs.dialog = Some(Dialog::About),
            Act::AiToggle => self.ai.open = !self.ai.open,
            Act::AiSend => self.ai_send(),
            Act::AiNew => self.ai_new(),
            Act::AiStop => self.ai_stop(),
            Act::AiOpenSql(sql) => self.ai_open_sql(sql),
            Act::AiChangeAnswer(ok) => self.ai_change_answer(ok),
            Act::AiAttachFile(path) => self.ai_attach(image_from_file(&path)),
            Act::AiAttachPixels(img) => self.ai_attach_pixels(&img),
            Act::AiPickImages => self.ai_pick_images(),
            Act::AiRemoveAttachment(i) => {
                if i < self.ai.attachments.len() {
                    self.ai.attachments.remove(i);
                }
            }
            Act::ShowAiSettings => self.show_ai_settings(),
            Act::SaveAiSettings(cfg) => self.save_ai_settings(cfg),
            Act::TestAi(cfg) => self.test_ai(cfg),
        }
    }
}
