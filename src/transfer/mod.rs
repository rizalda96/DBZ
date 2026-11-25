//! Moving data between a connection and files: export, import, SQL dumps, archive
//! dumps and restores (through the server's tools) and running SQL files. Export formats are [`Exporter`] strategies picked by [`ExportFormat`].

mod archive;
mod csv;
mod dump;
mod export;
mod html;
mod import;
mod json;
mod run_file;

pub use self::csv::parse_csv;
pub use archive::{dump_archive, restore_archive, ArchivePart};
pub use dump::dump;
pub use export::{export, ExportFormat, ExportOptions, Exporter};
pub use import::{import, ImportOptions};
pub use run_file::run_file;

use crate::db::DbError;

fn read_file(path: &str) -> Result<String, DbError> {
    std::fs::read_to_string(path).map_err(|e| DbError::Io(format!("Could not read {}: {}", path, e)))
}

fn write_file(path: &str, text: String) -> Result<(), DbError> {
    std::fs::write(path, text).map_err(|e| DbError::Io(format!("Could not write {}: {}", path, e)))
}
