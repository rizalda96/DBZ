//! The `Exporter` strategy and the export task.

use super::csv::DelimitedExporter;
use super::html::HtmlExporter;
use super::json::JsonExporter;
use super::write_file;
use crate::db::{DbError, Driver};
use crate::domain::ResultSet;
use crate::sql::{insert_row, Dialect};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExportFormat {
    Csv,
    Tsv,
    Json,
    Sql,
    Html,
}

/// What to export and where. Shared by the dialog and the worker request.
#[derive(Clone, Debug)]
pub struct ExportOptions {
    pub table: String,
    pub format: ExportFormat,
    pub path: String,
    pub header: bool,
}

/// Turns a result set into the text of one file format.
pub trait Exporter {
    fn render(&self, table: &str, rs: &ResultSet, header: bool, dialect: &dyn Dialect) -> String;
}

struct SqlExporter;

impl Exporter for SqlExporter {
    fn render(&self, table: &str, rs: &ResultSet, _header: bool, dialect: &dyn Dialect) -> String {
        rs.rows.iter().map(|r| format!("{};\n", insert_row(dialect, table, &rs.columns, r))).collect()
    }
}

static CSV: DelimitedExporter = DelimitedExporter::CSV;
static TSV: DelimitedExporter = DelimitedExporter::TSV;

impl ExportFormat {
    pub const ALL: [ExportFormat; 5] = [ExportFormat::Csv, ExportFormat::Tsv, ExportFormat::Json, ExportFormat::Sql, ExportFormat::Html];

    pub fn label(self) -> &'static str {
        match self {
            ExportFormat::Csv => "CSV file (*.csv)",
            ExportFormat::Tsv => "Text file, tab separated (*.txt)",
            ExportFormat::Json => "JSON file (*.json)",
            ExportFormat::Sql => "SQL script (*.sql)",
            ExportFormat::Html => "HTML table (*.html)",
        }
    }

    pub fn ext(self) -> &'static str {
        match self {
            ExportFormat::Csv => "csv",
            ExportFormat::Tsv => "txt",
            ExportFormat::Json => "json",
            ExportFormat::Sql => "sql",
            ExportFormat::Html => "html",
        }
    }

    /// The one place that picks an exporter for a format.
    pub fn exporter(self) -> &'static dyn Exporter {
        match self {
            ExportFormat::Csv => &CSV,
            ExportFormat::Tsv => &TSV,
            ExportFormat::Json => &JsonExporter,
            ExportFormat::Sql => &SqlExporter,
            ExportFormat::Html => &HtmlExporter,
        }
    }
}

/// Writes every row of a table to a file.
pub fn export(driver: &mut dyn Driver, db: &str, opts: &ExportOptions) -> Result<String, DbError> {
    let dialect = driver.dialect();
    let rs = driver.first_set(db, &format!("SELECT * FROM {}", dialect.quote_ident(&opts.table)))?;
    let text = opts.format.exporter().render(&opts.table, &rs, opts.header, dialect);
    write_file(&opts.path, text)?;
    Ok(format!("Exported {} record(s) from {} to {}", rs.rows.len(), opts.table, opts.path))
}
