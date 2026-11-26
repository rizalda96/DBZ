//! Saved connections of other database tools (Navicat, HeidiSQL, DBeaver), passwords
//! included. Each tool is a [`ConnImporter`] strategy picked by [`ConnSource`].

mod aes_cbc;
mod dbeaver;
mod heidisql;
mod navicat;
mod navicat_cipher;
mod navicat_cred;
mod registry;
mod xml_attrs;

#[cfg(test)]
mod tests;

use std::path::Path;

use crate::db::DbError;
use crate::domain::ConnConfig;

/// Connections read from another tool. Their ids are 0; the app gives them real ones.
#[derive(Debug, Default)]
pub struct Imported {
    pub conns: Vec<ConnConfig>,
    /// "name — reason" for each connection that could not be taken over.
    pub skipped: Vec<String>,
}

impl Imported {
    fn add(&mut self, name: &str, conn: Result<ConnConfig, &str>) {
        match conn {
            Ok(cfg) => self.conns.push(cfg),
            Err(reason) => self.skipped.push(format!("{} — {}", name, reason)),
        }
    }
}

/// Reads the connections one tool has saved.
pub trait ConnImporter: Sync {
    fn label(&self) -> &'static str;
    /// Where the file comes from, and what an empty path reads.
    fn hint(&self) -> &'static str;
    /// File picker filter: (name, extensions).
    fn filter(&self) -> (&'static str, &'static [&'static str]);
    /// `None` reads the tool's own default place (registry or settings folder).
    fn read(&self, path: Option<&Path>) -> Result<Imported, DbError>;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConnSource {
    Navicat,
    HeidiSql,
    DBeaver,
}

impl ConnSource {
    pub const ALL: [ConnSource; 3] = [ConnSource::Navicat, ConnSource::HeidiSql, ConnSource::DBeaver];

    pub fn importer(self) -> &'static dyn ConnImporter {
        match self {
            ConnSource::Navicat => &navicat::Navicat,
            ConnSource::HeidiSql => &heidisql::HeidiSql,
            ConnSource::DBeaver => &dbeaver::DBeaver,
        }
    }
}

/// Reads `source`'s connections from `path`, or from its default place when `path` is blank.
pub fn import_connections(source: ConnSource, path: &str) -> Result<Imported, DbError> {
    let path = path.trim();
    source.importer().read((!path.is_empty()).then(|| Path::new(path)))
}

/// A settings file as text: UTF-8 (with or without BOM) or UTF-16 LE with BOM.
fn read_text(path: &Path) -> Result<String, DbError> {
    let bytes = std::fs::read(path).map_err(|e| DbError::Io(format!("Could not read {}: {}", path.display(), e)))?;
    Ok(decode_text(&bytes))
}

fn decode_text(bytes: &[u8]) -> String {
    if let Some(wide) = bytes.strip_prefix(&[0xFF, 0xFE]) {
        let units: Vec<u16> = wide.chunks_exact(2).map(|p| u16::from_le_bytes([p[0], p[1]])).collect();
        return String::from_utf16_lossy(&units);
    }
    String::from_utf8_lossy(bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(bytes)).into_owned()
}

/// A named value of a registry key or XML element; names are compared without case.
fn value<'a>(pairs: &'a [(String, String)], name: &str) -> Option<&'a str> {
    pairs.iter().find(|(n, _)| n.eq_ignore_ascii_case(name)).map(|(_, v)| v.as_str())
}

/// A decimal or `0x` hex number, as registry dumps and settings files write them.
fn parse_num(s: &str) -> Option<u64> {
    let s = s.trim();
    match s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
        Some(hex) => u64::from_str_radix(hex, 16).ok(),
        None => s.parse().ok(),
    }
}

fn parse_port(s: &str) -> Option<u16> {
    parse_num(s).and_then(|n| u16::try_from(n).ok()).filter(|p| *p != 0)
}

fn from_hex(s: &str) -> Option<Vec<u8>> {
    let s = s.trim();
    if s.len() % 2 != 0 || !s.is_ascii() {
        return None;
    }
    (0..s.len()).step_by(2).map(|i| u8::from_str_radix(&s[i..i + 2], 16).ok()).collect()
}
