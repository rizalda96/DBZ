//! Open dialogs and what the user has typed into them.

use super::actions::Confirm;
use crate::domain::ConnConfig;
use crate::transfer::{ExportOptions, ImportOptions};

pub enum Dialog {
    Confirm { title: String, msg: String, action: Confirm },
    NewDb { cid: u64, name: String, charset: String },
    Export { cid: u64, db: String, opts: ExportOptions },
    Import { cid: u64, db: String, opts: ImportOptions },
    RunFile { cid: u64, db: String, path: String, cont: bool },
    SaveQuery { tab: u64, name: String },
    Rename { cid: u64, db: String, table: String, new: String },
    About,
}

/// The New / Edit Connection window.
pub struct ConnDialog {
    /// The connection being edited; None for a new one.
    pub editing: Option<u64>,
    pub cfg: ConnConfig,
    pub page: usize,
    pub test: Option<Result<String, String>>,
    pub test_id: Option<u64>,
}

impl ConnDialog {
    pub fn new(editing: Option<u64>, cfg: ConnConfig) -> Self {
        ConnDialog { editing, cfg, page: 0, test: None, test_id: None }
    }
}

#[derive(Default)]
pub struct DialogState {
    pub alert: Option<(String, String)>,
    pub dialog: Option<Dialog>,
    pub conn: Option<ConnDialog>,
}

impl DialogState {
    pub fn any_open(&self) -> bool {
        self.alert.is_some() || self.dialog.is_some() || self.conn.is_some()
    }
}
