//! File work: export/import wizards, dumps, backups, SQL files and the saved-file lists.

use super::persistence::Store;
use super::{Confirm, Dbz, Dialog, ObjRef};
use crate::shared::{default_dir, now_string, sanitize};
use crate::transfer::{ExportFormat, ExportOptions, ImportOptions};
use crate::worker::Req;

impl Dbz {
    pub(super) fn export_dialog(&mut self, obj: ObjRef) {
        let path = default_dir().join(format!("{}.csv", sanitize(&obj.name))).to_string_lossy().to_string();
        let opts = ExportOptions { table: obj.name, format: ExportFormat::Csv, path, header: true };
        self.dialogs.dialog = Some(Dialog::Export { cid: obj.cid, db: obj.db, opts });
    }

    pub(super) fn import_dialog(&mut self, cid: u64, db: String, table: Option<String>) {
        let opts = ImportOptions { create: table.is_none(), table: table.unwrap_or_default(), path: String::new(), header: true, delim: ',' };
        self.dialogs.dialog = Some(Dialog::Import { cid, db, opts });
    }

    /// Asks where to save, then dumps the database there.
    pub(super) fn dump(&mut self, cid: u64, db: String, data: bool) {
        let file = rfd::FileDialog::new().set_title("Dump SQL File").add_filter("SQL file", &["sql"]).set_file_name(format!("{}.sql", sanitize(&db))).save_file();
        let Some(p) = file else { return };
        self.status = format!("Dumping {}…", db);
        self.send(cid, Req::Dump { db, path: p.to_string_lossy().to_string(), data });
    }

    /// Dumps the database (with data) into its backups folder, named by date and time.
    pub(super) fn new_backup(&mut self, cid: u64, db: String) {
        let dir = self.store.folder("backups", cid, &db);
        if let Err(e) = std::fs::create_dir_all(&dir) {
            return self.alert("Backup", e.to_string());
        }
        let stamp: String = now_string().chars().filter(|c| c.is_ascii_digit()).collect();
        let path = dir.join(format!("{}.sql", stamp));
        self.status = format!("Backing up {}…", db);
        self.send(cid, Req::Dump { db, path: path.to_string_lossy().to_string(), data: true });
    }

    pub(super) fn ask_restore_backup(&mut self, obj: ObjRef) {
        let msg = format!("Restore backup \"{}\" into \"{}\"? Tables in the backup will be replaced.", obj.name, obj.db);
        self.confirm("Restore Backup", msg, Confirm::RestoreBackup(obj));
    }

    pub(super) fn ask_delete_backup(&mut self, obj: ObjRef) {
        let msg = format!("Delete the backup \"{}\"?", obj.name);
        self.confirm("Delete Backup", msg, Confirm::DeleteBackup(obj));
    }

    pub(super) fn restore_backup(&mut self, obj: ObjRef) {
        let path = self.store.folder("backups", obj.cid, &obj.db).join(format!("{}.sql", obj.name));
        self.status = format!("Restoring {}…", obj.name);
        self.send(obj.cid, Req::RunFile { db: obj.db, path: path.to_string_lossy().to_string(), continue_on_error: true });
    }

    /// Deletes a saved query or backup file.
    pub(super) fn delete_saved(&mut self, kind: &str, obj: ObjRef) {
        let path = self.store.folder(kind, obj.cid, &obj.db).join(format!("{}.sql", obj.name));
        if let Err(e) = std::fs::remove_file(&path) {
            self.alert("Delete", format!("Could not delete {}: {}", path.display(), e));
        }
        self.scan_files(obj.cid, &obj.db);
        self.ws.sel.obj = None;
    }

    /// File › Open SQL File: picks a file and opens it in a new query tab.
    pub(super) fn open_sql_file(&mut self) {
        let Some(p) = rfd::FileDialog::new().add_filter("SQL file", &["sql", "txt"]).pick_file() else { return };
        let sql = match std::fs::read_to_string(&p) {
            Ok(sql) => sql,
            Err(e) => return self.alert("Open SQL File", e.to_string()),
        };
        if let Some(cid) = self.ws.query_target() {
            self.new_query(cid, self.ws.sel.db.clone().unwrap_or_default(), sql);
        }
    }

    /// Re-reads the saved queries and backups of a database.
    pub(super) fn scan_files(&mut self, cid: u64, db: &str) {
        let queries = Store::list_sql_files(&self.store.folder("queries", cid, db), true);
        let backups = Store::list_sql_files(&self.store.folder("backups", cid, db), false);
        if let Some(d) = self.ws.db_mut(cid, db) {
            d.queries = queries;
            d.backups = backups;
        }
    }
}
