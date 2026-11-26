//! HeidiSQL: its sessions in the Windows registry, or a settings file (File › Export
//! settings, or `portable_settings.txt`) with lines `Servers\<folders>\<session>\<name><|||><type><|||><value>`.

use std::path::Path;

use super::registry::{self, RegKey};
use super::{parse_num, parse_port, read_text, value, ConnImporter, Imported};
use crate::db::DbError;
use crate::domain::{ConnConfig, DbKind, SqlAuth};

const REG_ROOT: &str = r"HKEY_CURRENT_USER\Software\HeidiSQL\Servers";
const SEP: &str = "<|||>";
const CR: &str = "<{{{>";
const LF: &str = "<}}}>";

pub(super) struct HeidiSql;

impl ConnImporter for HeidiSql {
    fn label(&self) -> &'static str {
        "HeidiSQL"
    }

    fn hint(&self) -> &'static str {
        "A file from HeidiSQL's File › Export settings, or portable_settings.txt of a portable copy. Leave it empty to read the sessions HeidiSQL keeps in the Windows registry."
    }

    fn filter(&self) -> (&'static str, &'static [&'static str]) {
        ("HeidiSQL settings", &["txt"])
    }

    fn read(&self, path: Option<&Path>) -> Result<Imported, DbError> {
        let keys = match path {
            Some(p) => settings_keys(&read_text(p)?),
            None => registry::read(REG_ROOT)?,
        };
        Ok(sessions(&keys))
    }
}

/// The `Servers\…` lines of a settings file, grouped per key like the registry.
pub(super) fn settings_keys(text: &str) -> Vec<RegKey> {
    let mut keys: Vec<RegKey> = vec![];
    for line in text.lines() {
        let mut parts = line.splitn(3, SEP);
        let (Some(full), Some(_kind), Some(data)) = (parts.next(), parts.next(), parts.next()) else { continue };
        let Some((path, name)) = full.strip_prefix(r"Servers\").and_then(|p| p.rsplit_once('\\')) else { continue };
        let pair = (name.to_string(), data.replace(CR, "\r").replace(LF, "\n"));
        match keys.iter_mut().find(|k| k.path == path) {
            Some(key) => key.values.push(pair),
            None => keys.push(RegKey { path: path.to_string(), values: vec![pair] }),
        }
    }
    keys
}

/// Sessions are the keys with a host; folders only group them.
pub(super) fn sessions(keys: &[RegKey]) -> Imported {
    let mut out = Imported::default();
    for key in keys.iter().filter(|k| value(&k.values, "Host").is_some()) {
        out.add(key.name(), session(key));
    }
    out
}

fn session(key: &RegKey) -> Result<ConnConfig, &'static str> {
    let get = |n: &str| value(&key.values, n).unwrap_or_default();
    let kind = net_kind(parse_num(get("NetType")).unwrap_or(0)).ok_or("this database type is not supported")?;
    let mut cfg = ConnConfig::blank(0, kind);
    cfg.name = key.name().to_string();
    cfg.host = get("Host").to_string();
    cfg.port = parse_port(get("Port")).unwrap_or(cfg.port);
    cfg.user = get("User").to_string();
    cfg.password = decode_password(get("Password"));
    // "Databases" is the session's database filter; its first entry is the one to open.
    cfg.initial_db = get("Databases").split(';').next().unwrap_or_default().trim().to_string();
    if kind == DbKind::SqlServer && parse_num(get("WindowsAuth")) == Some(1) {
        cfg.auth = SqlAuth::Windows;
    }
    Ok(cfg)
}

/// HeidiSQL's `TNetType`: 0–2, 11 and 16 are MySQL/MariaDB (TCP, pipe, SSH tunnel,
/// ProxySQL, RDS), 3–7 SQL Server, 8–9 PostgreSQL. The rest (SQLite, Interbase, Firebird…)
/// DBZ doesn't speak.
fn net_kind(net_type: u64) -> Option<DbKind> {
    match net_type {
        0..=2 | 11 | 16 => Some(DbKind::MySql),
        3..=7 => Some(DbKind::SqlServer),
        8 | 9 => Some(DbKind::PostgreSql),
        _ => None,
    }
}

/// HeidiSQL writes each character as hex of its code plus a salt (1–9) and puts the
/// salt digit last: two hex digits per character wrapping past 255, or, when the text
/// ends in an extra `0`, four per UTF-16 unit wrapping past 65535.
pub(super) fn decode_password(text: &str) -> String {
    let (body, wide) = match text.strip_suffix('0') {
        Some(rest) => (rest, true),
        None => (text, false),
    };
    let Some(salt) = body.chars().last().and_then(|c| c.to_digit(10)) else { return String::new() };
    let body = &body[..body.len() - 1];
    // HeidiSQL's own wrap: +255 for bytes (not 256), +65536 for UTF-16 units.
    let (width, wrap) = if wide { (4, 65536) } else { (2, 255) };
    if body.len() % width != 0 || !body.is_ascii() {
        return String::new();
    }
    let unit = |i: usize| {
        let code = u32::from_str_radix(&body[i..i + width], 16).ok()?;
        u16::try_from(if code < salt { code + wrap - salt } else { code - salt }).ok()
    };
    let units: Option<Vec<u16>> = (0..body.len()).step_by(width).map(unit).collect();
    units.map(|u| String::from_utf16_lossy(&u)).unwrap_or_default()
}
